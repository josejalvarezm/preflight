//! Pure policy evaluation benchmark — no audit-log I/O, no routing.
//!
//! Calls `engine.evaluate()` directly to measure the cost of keyword index
//! lookup + two-phase boundary matching in isolation. This separates policy
//! evaluation latency from the file-write overhead in `kernel.route()`.
//!
//! Run with: cargo bench -p ai-os-kernel --bench pure_eval_bench

use ai_os_kernel::policy::{PolicyEngine, PolicyVerdict};
use ai_os_shared::boundary::{BoundaryCategory, PolicyBoundary};
use ai_os_shared::task::TaskDescriptor;
use chrono::Utc;
use std::time::Instant;

const ITERATIONS: usize = 1000;

fn build_boundaries() -> Vec<PolicyBoundary> {
    vec![
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
    ]
}

fn mean(vals: &[f64]) -> f64 {
    vals.iter().sum::<f64>() / vals.len() as f64
}

fn std_dev(vals: &[f64], m: f64) -> f64 {
    if vals.len() < 2 { return 0.0; }
    (vals.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (vals.len() - 1) as f64).sqrt()
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() { return 0.0; }
    let idx = (p / 100.0 * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn main() {
    let engine = PolicyEngine::from_boundaries(build_boundaries());

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
}
