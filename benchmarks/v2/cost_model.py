"""
Cost model: per-1M-decision cost and throughput comparison.

Uses measured latencies from the paper experiments and published
OpenAI API pricing (April 2026) to compute the per-1M-decision
cost for different enforcement architectures.

Assumptions are stated explicitly. Run to reproduce the table.

Usage:
    python benchmarks/v2/cost_model.py
"""

import json
import os

# ---------------------------------------------------------------------------
# Measured latencies (from paper Tables 1-3 and SetFit results)
# ---------------------------------------------------------------------------

# Mean latencies in milliseconds
AIOS_REFUSE_MS  = 0.118       # 118 µs
AIOS_ALLOW_MS   = 0.113       # 113 µs
NEMO_REFUSE_MS  = 437.0
NEMO_ALLOW_MS   = 6927.0      # includes response generation
LLM_BIN_REFUSE  = 1063.0
LLM_BIN_ALLOW   = 385.3
LLM_VERB_REFUSE = 2582.7
LLM_VERB_ALLOW  = 418.0
SETFIT_SINGLE   = 6.15        # six-boundary: single-item CPU mean
SETFIT_BATCH    = 1.10        # six-boundary: per-item in batch=32

# Measured fast-path rates (six-boundary configuration,
# BOUNDARY-001..005 + OWASP LLM06)
AIOS_ATTACK_REFUSE = 0.087    # 124/1423 adversarial rephrasings caught
AIOS_SAFE_FPR      = 0.034    # 17/500 safe (Dolly 500) falsely refused

# ---------------------------------------------------------------------------
# Published API pricing (OpenAI, April 2026)
#   GPT-5.4 nano: $0.20/1M input, $1.25/1M output  (cheapest)
#   GPT-5.4 mini: $0.75/1M input, $4.50/1M output   (mid-tier)
# Self-hosted (A100 80GB cloud, on-demand ~$2.50/hr)
# ---------------------------------------------------------------------------

PRICING = {
    "GPT-5.4 nano": {"input_per_M": 0.20, "output_per_M": 1.25},
    "GPT-5.4 mini": {"input_per_M": 0.75, "output_per_M": 4.50},
}

# Token estimates per guardrail call
TOKENS_INPUT      = 180   # system prompt (~150) + user task (~30)
TOKENS_OUTPUT_NEMO = 20   # short classification response
TOKENS_OUTPUT_BIN  = 1    # single Y/N token

# Self-hosted GPU cost
GPU_COST_PER_HR = 2.50    # A100 80GB cloud on-demand (typical 2026 pricing)
NEMO_THROUGHPUT_PER_S = 2.3   # measured: ~2.3 tasks/s (refuse path)

# ---------------------------------------------------------------------------
# Traffic mix assumption
# ---------------------------------------------------------------------------
# Conservative: 95% benign, 5% policy-violating (of which most are naive)
SAFE_FRAC   = 0.95
ATTACK_FRAC = 0.05


def cost_per_M_decisions_api(tokens_in, tokens_out, pricing_tier):
    """Cost for 1M LLM guardrail calls via API."""
    p = PRICING[pricing_tier]
    return tokens_in * p["input_per_M"] + tokens_out * p["output_per_M"]


def cost_self_hosted(throughput_per_s, gpu_cost_hr):
    """Cost for 1M decisions on self-hosted GPU."""
    tasks_per_hr = throughput_per_s * 3600
    hours_needed = 1_000_000 / tasks_per_hr
    return hours_needed * gpu_cost_hr


def throughput(mean_latency_ms):
    """Single-stream throughput in requests/second."""
    return 1000.0 / mean_latency_ms


def main():
    sep = "=" * 80
    print(sep)
    print("  COST MODEL: PER-1M-DECISION COST AND THROUGHPUT")
    print(sep)

    # --- Compute fast-path rate ---
    fp_rate = SAFE_FRAC * AIOS_SAFE_FPR + ATTACK_FRAC * AIOS_ATTACK_REFUSE
    llm_frac = 1.0 - fp_rate
    print(f"\n  Traffic mix assumption: {SAFE_FRAC*100:.0f}% safe, {ATTACK_FRAC*100:.0f}% attack")
    print(f"  AI-OS fast-path rate: {fp_rate*100:.1f}% (blocked without LLM)")
    print(f"  Fraction needing LLM: {llm_frac*100:.1f}%")

    # --- Per-architecture costs (API) ---
    print(f"\n  {'Architecture':<35} {'nano ($)':<12} {'mini ($)':<12} {'Self-hosted ($)':<15}")
    print(f"  {'-'*35} {'-'*12} {'-'*12} {'-'*15}")

    rows = []

    # 1. NeMo alone (every task through LLM)
    for tier in PRICING:
        cost = cost_per_M_decisions_api(TOKENS_INPUT, TOKENS_OUTPUT_NEMO, tier)
        if tier == "GPT-5.4 nano":
            nano_nemo = cost
        else:
            mini_nemo = cost
    self_nemo = cost_self_hosted(NEMO_THROUGHPUT_PER_S, GPU_COST_PER_HR)
    rows.append(("NeMo alone", nano_nemo, mini_nemo, self_nemo))

    # 2. LLM-binary alone
    for tier in PRICING:
        cost = cost_per_M_decisions_api(TOKENS_INPUT, TOKENS_OUTPUT_BIN, tier)
        if tier == "GPT-5.4 nano":
            nano_bin = cost
        else:
            mini_bin = cost
    bin_tps = throughput((LLM_BIN_REFUSE + LLM_BIN_ALLOW) / 2)
    self_bin = cost_self_hosted(bin_tps, GPU_COST_PER_HR)
    rows.append(("LLM-binary alone", nano_bin, mini_bin, self_bin))

    # 3. SetFit alone
    rows.append(("SetFit alone (CPU)", 0.0, 0.0, 0.0))

    # 4. AI-OS alone
    rows.append(("AI-OS alone (CPU)", 0.0, 0.0, 0.0))

    # 5. AI-OS -> NeMo layered
    nano_layered = llm_frac * nano_nemo
    mini_layered = llm_frac * mini_nemo
    self_layered = llm_frac * self_nemo
    rows.append(("AI-OS -> NeMo", nano_layered, mini_layered, self_layered))

    # 6. AI-OS -> SetFit -> NeMo three-tier
    # SetFit handles remaining traffic; only explicit failures escalate to NeMo.
    # SetFit recall on auto corpus: 100% (attacks caught by SetFit, no NeMo needed
    #   for attacks). SetFit FPR: 0% -> no extra NeMo calls from false positives.
    # But SetFit was trained on same boundaries. On truly novel attacks, assume
    # SetFit catches 90% and remaining 10% escalate to NeMo.
    setfit_miss_rate = 0.10  # conservative: 10% of remaining attacks miss SetFit
    nemo_frac_3tier = ATTACK_FRAC * (1 - AIOS_ATTACK_REFUSE) * setfit_miss_rate
    nano_3tier = nemo_frac_3tier * nano_nemo
    mini_3tier = nemo_frac_3tier * mini_nemo
    self_3tier = nemo_frac_3tier * self_nemo
    rows.append(("AI-OS -> SetFit -> NeMo", nano_3tier, mini_3tier, self_3tier))

    for name, nano, mini, selfh in rows:
        print(f"  {name:<35} {nano:<12.2f} {mini:<12.2f} {selfh:<15.2f}")

    # --- Throughput ---
    print(f"\n  {'System':<30} {'Throughput (req/s)':<20} {'Latency (ms)':<15}")
    print(f"  {'-'*30} {'-'*20} {'-'*15}")

    tp_data = [
        ("AI-OS (release)", throughput(AIOS_REFUSE_MS), AIOS_REFUSE_MS),
        ("SetFit (single, CPU)", throughput(SETFIT_SINGLE), SETFIT_SINGLE),
        ("SetFit (batch=32, CPU)", throughput(SETFIT_BATCH), SETFIT_BATCH),
        ("LLM-binary", throughput((LLM_BIN_REFUSE + LLM_BIN_ALLOW) / 2),
         (LLM_BIN_REFUSE + LLM_BIN_ALLOW) / 2),
        ("NeMo Guardrails", throughput(NEMO_REFUSE_MS), NEMO_REFUSE_MS),
        ("LLM-verbose", throughput((LLM_VERB_REFUSE + LLM_VERB_ALLOW) / 2),
         (LLM_VERB_REFUSE + LLM_VERB_ALLOW) / 2),
    ]

    for name, tps, lat in tp_data:
        print(f"  {name:<30} {tps:<20,.1f} {lat:<15.1f}")

    # --- Layered throughput ---
    # layered: AI-OS handles fp_rate fraction at AI-OS speed,
    # rest at NeMo speed.
    layered_tps = 1000.0 / (fp_rate * AIOS_REFUSE_MS + llm_frac * NEMO_REFUSE_MS)
    print(f"\n  AI-OS -> NeMo layered throughput: {layered_tps:,.1f} req/s")
    print(f"    (weighted: {fp_rate*100:.1f}% at AI-OS speed + {llm_frac*100:.1f}% at NeMo speed)")

    # --- Write results ---
    results = {
        "assumptions": {
            "traffic_mix": f"{SAFE_FRAC*100:.0f}% safe, {ATTACK_FRAC*100:.0f}% attack",
            "tokens_input_per_call": TOKENS_INPUT,
            "tokens_output_nemo": TOKENS_OUTPUT_NEMO,
            "tokens_output_binary": TOKENS_OUTPUT_BIN,
            "gpu_cost_per_hr": GPU_COST_PER_HR,
            "aios_fast_path_rate": round(fp_rate, 4),
            "setfit_escalation_rate": setfit_miss_rate,
            "pricing_source": "OpenAI API (April 2026)",
            "self_hosted_gpu": "A100 80GB cloud on-demand",
        },
        "cost_per_1M_decisions": {r[0]: {"nano": round(r[1], 2), "mini": round(r[2], 2),
                                          "self_hosted": round(r[3], 2)} for r in rows},
        "throughput": {t[0]: {"req_per_s": round(t[1], 1), "latency_ms": round(t[2], 1)}
                       for t in tp_data},
        "layered_throughput_req_s": round(layered_tps, 1),
    }

    out_dir = os.path.join("paper", "v2_results", "cost_model")
    os.makedirs(out_dir, exist_ok=True)
    out_path = os.path.join(out_dir, "results.json")
    with open(out_path, "w") as f:
        json.dump(results, f, indent=2)
    print(f"\n  Results written to {out_path}")
    print(sep)


if __name__ == "__main__":
    main()
