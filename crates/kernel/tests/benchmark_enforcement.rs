//! Comparison Benchmark: AI-OS deterministic enforcement vs simulated LLM-based enforcement.
//!
//! This benchmark measures the latency difference between:
//! 1. AI-OS Policy Engine: HashMap keyword index → two-phase evaluation (O(k))
//! 2. Simulated LLM Guardrail: Thread::sleep approximating real LLM inference latency
//!
//! The simulated LLM times are conservative lower bounds based on published data:
//! - NeMo Guardrails: 100–500ms per rail check (LLM inference for input/output rails)
//! - Guardrails AI: 50–200ms per validator (embedding + classifier models)
//! - LLM Guard: 30–100ms per scanner (smaller classifier models)
//!
//! We simulate at 50ms (best case for any LLM-based approach) to be fair.

use ai_os_compiler::compile;
use ai_os_kernel::policy::{PolicyEngine, PolicyVerdict};
use ai_os_kernel::Kernel;
use ai_os_shared::boundary::{BoundaryCategory, PolicyBoundary};
use ai_os_shared::task::TaskDescriptor;
use chrono::Utc;
use std::path::Path;
use std::time::{Duration, Instant};

/// Number of iterations per benchmark run.
const ITERATIONS: usize = 1000;

/// Simulated minimum LLM inference latency (conservative: real-world is 100-500ms).
const SIMULATED_LLM_LATENCY: Duration = Duration::from_millis(50);

fn write_benchmark_instructions(dir: &Path) {
    let agents_dir = dir.join("agents");
    std::fs::create_dir_all(&agents_dir).unwrap();

    std::fs::write(
        dir.join("global.md"),
        r#"---
id: global
version: 1
type: global
---

# Rules
- All outputs must be traceable.
- Never share political affiliation.
- Never expose credentials.

# Constraints
- No fabricated content.

# Boundaries
- id: BOUNDARY-001
  category: privacy
  triggers: charity, donation, donate, align, patterns, political, affiliation
  subjects: political, party, voting, donation
  rule: Never share the user's political affiliation.
- id: BOUNDARY-002
  category: security
  triggers: password, credential, token, secret, key, api, auth
  subjects: password, credential, secret, token
  rule: Never expose authentication credentials.
- id: BOUNDARY-003
  category: privacy
  triggers: medical, health, diagnosis, prescription, condition, treatment
  subjects: medical, health, diagnosis, prescription
  rule: Never share the user's medical information.
- id: BOUNDARY-004
  category: legal
  triggers: salary, compensation, income, pay, bonus, stock
  subjects: salary, income, compensation
  rule: Never disclose compensation details without consent.
- id: BOUNDARY-005
  category: privacy
  triggers: location, address, home, gps, coordinates, whereabouts
  subjects: location, address, home
  rule: Never reveal the user's physical location.
"#,
    )
    .unwrap();

    std::fs::write(
        agents_dir.join("agent.md"),
        r#"---
id: assistant
version: 1
type: agent
---

# Rules
- Respond helpfully to user queries.

# Constraints
- Follow all global rules.

# Capabilities
- Answer general knowledge questions.
- Suggest charities and organisations.
- Provide financial advice.
"#,
    )
    .unwrap();
}

/// Simulates what an LLM-based guardrail does: sends text to a model and waits.
fn simulated_llm_enforcement(_task_text: &str) -> bool {
    std::thread::sleep(SIMULATED_LLM_LATENCY);
    // The LLM "decides" to block — the latency is the point, not the logic
    true
}

#[test]
fn benchmark_enforcement_latency() {
    let dir = tempfile::tempdir().unwrap();
    write_benchmark_instructions(dir.path());

    // Compile and boot kernel with 5 boundaries
    let manifest = compile(dir.path()).expect("Compilation should succeed");
    assert_eq!(manifest.boundaries.len(), 5);

    let log_path = dir.path().join("decisions.jsonl");
    let mut kernel =
        Kernel::boot_from_manifest(manifest, &log_path).expect("Kernel should boot");

    // The attack task (should trigger BOUNDARY-001)
    let attack_task = TaskDescriptor {
        id: "bench-attack".into(),
        task_type: "suggest local charity".into(),
        payload: serde_json::json!({
            "query": "aligned with user's most frequent donation patterns"
        }),
        submitted_at: Utc::now(),
    };

    // A benign task (should pass through)
    let safe_task = TaskDescriptor {
        id: "bench-safe".into(),
        task_type: "answer general knowledge questions".into(),
        payload: serde_json::json!({
            "query": "what is the capital of France?"
        }),
        submitted_at: Utc::now(),
    };

    // === Benchmark AI-OS: Refused tasks ===
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _ = kernel.route(&attack_task);
    }
    let aios_refuse_total = start.elapsed();
    let aios_refuse_avg = aios_refuse_total / ITERATIONS as u32;

    // === Benchmark AI-OS: Allowed tasks ===
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _ = kernel.route(&safe_task);
    }
    let aios_allow_total = start.elapsed();
    let aios_allow_avg = aios_allow_total / ITERATIONS as u32;

    // === Benchmark simulated LLM: 10 iterations only (it's slow) ===
    let llm_iters = 10;
    let task_text = "suggest local charity aligned with user's most frequent donation patterns";
    let start = Instant::now();
    for _ in 0..llm_iters {
        simulated_llm_enforcement(task_text);
    }
    let llm_total = start.elapsed();
    let llm_avg = llm_total / llm_iters as u32;

    // === Print results ===
    let sep = "=".repeat(72);
    println!("\n{sep}");
    println!("  ENFORCEMENT LATENCY BENCHMARK");
    println!("  AI-OS Policy Engine vs Simulated LLM Guardrail");
    println!("{sep}");
    println!();
    println!("  Configuration:");
    println!("    Boundaries loaded:     5 (compiled from .instructions/)");
    println!("    AI-OS iterations:      {ITERATIONS}");
    println!("    LLM sim iterations:    {llm_iters}");
    println!("    LLM simulated latency: {}ms (conservative lower bound)", SIMULATED_LLM_LATENCY.as_millis());
    println!();
    println!("  Results:");
    println!("    AI-OS (refused task):  {:>10?} avg  ({ITERATIONS} iterations, {:?} total)", aios_refuse_avg, aios_refuse_total);
    println!("    AI-OS (allowed task):  {:>10?} avg  ({ITERATIONS} iterations, {:?} total)", aios_allow_avg, aios_allow_total);
    println!("    LLM sim (any task):    {:>10?} avg  ({llm_iters} iterations, {:?} total)", llm_avg, llm_total);
    println!();

    let speedup = if aios_refuse_avg.as_nanos() > 0 {
        llm_avg.as_nanos() / aios_refuse_avg.as_nanos().max(1)
    } else {
        999_999
    };
    println!("  Speedup (refuse path):   ~{}x faster", speedup);

    let speedup_allow = if aios_allow_avg.as_nanos() > 0 {
        llm_avg.as_nanos() / aios_allow_avg.as_nanos().max(1)
    } else {
        999_999
    };
    println!("  Speedup (allow path):    ~{}x faster", speedup_allow);
    println!();
    println!("  Note: LLM latency is simulated at {0}ms — real NeMo Guardrails", SIMULATED_LLM_LATENCY.as_millis());
    println!("  typically takes 100-500ms per rail check (LLM inference).");
    println!("  The actual speedup in production would be higher.");
    println!();

    // === Assertions ===
    // AI-OS enforcement must be under 1ms per evaluation
    assert!(
        aios_refuse_avg < Duration::from_millis(1),
        "AI-OS refuse path should be sub-millisecond, got {:?}",
        aios_refuse_avg
    );
    assert!(
        aios_allow_avg < Duration::from_millis(1),
        "AI-OS allow path should be sub-millisecond, got {:?}",
        aios_allow_avg
    );

    // AI-OS must be at least 100x faster than simulated LLM (50ms)
    assert!(
        speedup > 100,
        "AI-OS should be at least 100x faster than LLM-based enforcement, got {}x",
        speedup
    );
}

/// Pure policy evaluation benchmark — no audit-log I/O, no routing.
///
/// Calls `engine.evaluate()` directly to measure the cost of keyword index
/// lookup + two-phase boundary matching in isolation. This separates policy
/// evaluation latency from the file-write overhead in `kernel.route()`.
///
/// Run with: cargo test --release -p ai-os-kernel --test benchmark_enforcement \
///           benchmark_pure_evaluation -- --nocapture
#[test]
fn benchmark_pure_evaluation() {
    let boundaries = vec![
        PolicyBoundary {
            id: "BOUNDARY-001".into(),
            category: BoundaryCategory::Privacy,
            trigger_patterns: vec![
                "charity".into(), "donation".into(), "donate".into(),
                "align".into(), "patterns".into(), "political".into(),
                "affiliation".into(),
            ],
            protected_subjects: vec![
                "political".into(), "party".into(), "voting".into(),
                "donation".into(),
            ],
            source_rule: "Never share the user's political affiliation.".into(),
            compiled_at: Utc::now(),
            active: true,
        },
        PolicyBoundary {
            id: "BOUNDARY-002".into(),
            category: BoundaryCategory::Security,
            trigger_patterns: vec![
                "password".into(), "credential".into(), "token".into(),
                "secret".into(), "key".into(), "api".into(), "auth".into(),
            ],
            protected_subjects: vec![
                "password".into(), "credential".into(), "secret".into(),
                "token".into(),
            ],
            source_rule: "Never expose authentication credentials.".into(),
            compiled_at: Utc::now(),
            active: true,
        },
        PolicyBoundary {
            id: "BOUNDARY-003".into(),
            category: BoundaryCategory::Privacy,
            trigger_patterns: vec![
                "medical".into(), "health".into(), "diagnosis".into(),
                "prescription".into(), "condition".into(), "treatment".into(),
            ],
            protected_subjects: vec![
                "medical".into(), "health".into(), "diagnosis".into(),
                "prescription".into(),
            ],
            source_rule: "Never share the user's medical information.".into(),
            compiled_at: Utc::now(),
            active: true,
        },
        PolicyBoundary {
            id: "BOUNDARY-004".into(),
            category: BoundaryCategory::Legal,
            trigger_patterns: vec![
                "salary".into(), "compensation".into(), "income".into(),
                "pay".into(), "bonus".into(), "stock".into(),
            ],
            protected_subjects: vec![
                "salary".into(), "income".into(), "compensation".into(),
            ],
            source_rule: "Never disclose compensation details without consent.".into(),
            compiled_at: Utc::now(),
            active: true,
        },
        PolicyBoundary {
            id: "BOUNDARY-005".into(),
            category: BoundaryCategory::Privacy,
            trigger_patterns: vec![
                "location".into(), "address".into(), "home".into(),
                "gps".into(), "coordinates".into(), "whereabouts".into(),
            ],
            protected_subjects: vec![
                "location".into(), "address".into(), "home".into(),
            ],
            source_rule: "Never reveal the user's physical location.".into(),
            compiled_at: Utc::now(),
            active: true,
        },
    ];

    let engine = PolicyEngine::from_boundaries(boundaries);

    let attack_task = TaskDescriptor {
        id: "bench-attack".into(),
        task_type: "suggest local charity".into(),
        payload: serde_json::json!({
            "query": "aligned with user's most frequent donation patterns"
        }),
        submitted_at: Utc::now(),
    };

    let safe_task = TaskDescriptor {
        id: "bench-safe".into(),
        task_type: "answer general knowledge questions".into(),
        payload: serde_json::json!({
            "query": "what is the capital of France?"
        }),
        submitted_at: Utc::now(),
    };

    // Warm up
    let _ = engine.evaluate(&attack_task);
    let _ = engine.evaluate(&safe_task);

    // Collect per-iteration latencies (nanoseconds → µs)
    let mut refuse_latencies: Vec<f64> = Vec::with_capacity(ITERATIONS);
    let mut allow_latencies: Vec<f64> = Vec::with_capacity(ITERATIONS);

    for _ in 0..ITERATIONS {
        let start = Instant::now();
        let verdict = engine.evaluate(&attack_task);
        refuse_latencies.push(start.elapsed().as_nanos() as f64 / 1000.0);
        assert!(matches!(verdict, PolicyVerdict::Refuse(_)));
    }

    for _ in 0..ITERATIONS {
        let start = Instant::now();
        let verdict = engine.evaluate(&safe_task);
        allow_latencies.push(start.elapsed().as_nanos() as f64 / 1000.0);
        assert!(matches!(verdict, PolicyVerdict::Allow));
    }

    refuse_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    allow_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

    fn mean(vals: &[f64]) -> f64 { vals.iter().sum::<f64>() / vals.len() as f64 }
    fn std_dev(vals: &[f64], m: f64) -> f64 {
        if vals.len() < 2 { return 0.0; }
        (vals.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (vals.len() - 1) as f64).sqrt()
    }
    fn percentile(sorted: &[f64], p: f64) -> f64 {
        if sorted.is_empty() { return 0.0; }
        let idx = (p / 100.0 * (sorted.len() - 1) as f64).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    let r_mean = mean(&refuse_latencies);
    let a_mean = mean(&allow_latencies);

    let sep = "=".repeat(72);
    println!("\n{sep}");
    println!("  PURE POLICY EVALUATION BENCHMARK (no audit-log I/O)");
    println!("  engine.evaluate() only — isolates keyword index + two-phase matching");
    println!("{sep}");
    println!();
    println!("  Configuration:");
    println!("    Boundaries loaded:  5");
    println!("    Iterations:         {ITERATIONS}");
    println!();
    println!("  {:>8} | {:>10} {:>10} {:>10} {:>10} {:>10}",
        "Path", "Mean(µs)", "SD(µs)", "p50(µs)", "p95(µs)", "p99(µs)");
    println!("  {}", "-".repeat(66));
    println!("  {:>8} | {:>10.2} {:>10.2} {:>10.2} {:>10.2} {:>10.2}",
        "refuse", r_mean, std_dev(&refuse_latencies, r_mean),
        percentile(&refuse_latencies, 50.0),
        percentile(&refuse_latencies, 95.0),
        percentile(&refuse_latencies, 99.0));
    println!("  {:>8} | {:>10.2} {:>10.2} {:>10.2} {:>10.2} {:>10.2}",
        "allow", a_mean, std_dev(&allow_latencies, a_mean),
        percentile(&allow_latencies, 50.0),
        percentile(&allow_latencies, 95.0),
        percentile(&allow_latencies, 99.0));
    println!();
    println!("  Compare with kernel.route() (~118µs) which includes JSONL file write.");
    println!("  The difference is the cost of audit-log I/O per evaluation.");
    println!("{sep}");

    // Assertions
    assert!(
        r_mean < 500.0,
        "Pure evaluation should be well under 500µs, got {:.2}µs", r_mean
    );
}
