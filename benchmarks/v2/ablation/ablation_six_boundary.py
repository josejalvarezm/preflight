"""Six-boundary ablation: 4 matching modes on 1,423 attack corpus + Dolly-500 safe corpus."""
import csv, os, re, unicodedata, json
from collections import defaultdict

BOUNDARIES = {
    "BOUNDARY-001": {
        "triggers": ["charity", "donation", "donate", "align", "patterns",
                      "political", "affiliation"],
        "subjects": ["political", "party", "voting", "donation"],
    },
    "BOUNDARY-002": {
        "triggers": ["password", "credential", "token", "secret", "key",
                      "api", "auth"],
        "subjects": ["password", "credential", "secret", "token"],
    },
    "BOUNDARY-003": {
        "triggers": ["medical", "health", "diagnosis", "prescription",
                      "condition", "treatment"],
        "subjects": ["medical", "health", "diagnosis", "prescription"],
    },
    "BOUNDARY-004": {
        "triggers": ["salary", "compensation", "income", "pay", "bonus",
                      "stock"],
        "subjects": ["salary", "income", "compensation"],
    },
    "BOUNDARY-005": {
        "triggers": ["location", "address", "home", "gps", "coordinates",
                      "whereabouts"],
        "subjects": ["location", "address", "home"],
    },
    "BOUNDARY-006": {
        "triggers": ["proprietary", "confidential", "internal", "source",
                      "algorithm", "model", "weights", "training", "dataset",
                      "pii", "personal"],
        "subjects": ["proprietary", "confidential", "internal", "source",
                      "algorithm", "weights", "training", "dataset", "pii",
                      "personal", "identifiable"],
    },
}


def norm(w):
    return unicodedata.normalize("NFKC", w).lower()


def kw(text):
    return {norm(w) for w in re.findall(r"[a-zA-Z]+", norm(text))}


def check(kws, mode):
    for b in BOUNDARIES.values():
        t = {norm(x) for x in b["triggers"]}
        s = {norm(x) for x in b["subjects"]}
        ht = bool(kws & t)
        hs = bool(kws & s)
        if mode == "trigger" and ht: return True
        if mode == "subject" and hs: return True
        if mode == "intersect" and (ht and hs): return True
        if mode == "union" and (ht or hs): return True
    return False


ATK = os.path.join("benchmarks", "v2", "external_boundaries", "llm06",
                   "auto_rephrasings_llm06_results.csv")
SAFE = os.path.join("benchmarks", "v2", "safe_corpus", "dolly_500.csv")

with open(ATK, encoding="utf-8") as f:
    atk_rows = list(csv.DictReader(f))
with open(SAFE, encoding="utf-8") as f:
    safe_rows = list(csv.DictReader(f))

print(f"Attack corpus: {len(atk_rows)}  Safe corpus: {len(safe_rows)}")

results = {}
for mode in ("trigger", "subject", "intersect", "union"):
    a_hit = sum(1 for r in atk_rows if check(kw(r["rephrased"]), mode))
    s_hit = sum(1 for r in safe_rows if check(kw(r["instruction"]), mode))
    a_pct = a_hit / len(atk_rows) * 100
    s_pct = s_hit / len(safe_rows) * 100
    results[mode] = {"atk": (a_hit, len(atk_rows), a_pct),
                     "safe": (s_hit, len(safe_rows), s_pct)}
    print(f"{mode:>10}: attack {a_hit}/{len(atk_rows)} ({a_pct:.1f}%)  "
          f"safe {s_hit}/{len(safe_rows)} ({s_pct:.1f}%)")

out = os.path.join("paper", "v2_results", "ablation_six_boundary.json")
os.makedirs(os.path.dirname(out), exist_ok=True)
with open(out, "w", encoding="utf-8") as f:
    json.dump(results, f, indent=2)
print(f"\nSaved: {out}")
