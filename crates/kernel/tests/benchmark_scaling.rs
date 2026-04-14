//! Scaling study: enforcement latency vs number of boundaries.
//!
//! Validates the O(k) claim — that evaluation time scales with task keyword
//! count, not with the number of boundaries in the engine.
//!
//! Generates synthetic boundary sets of 5, 50, 100, 200, and 500 boundaries,
//! each with unique trigger/subject keywords. Measures enforcement latency
//! for the same 10 tasks at each scale.
//!
//! Run with: cargo test --release -p ai-os-kernel --test benchmark_scaling -- --nocapture

use ai_os_kernel::policy::{PolicyEngine, PolicyVerdict};
use ai_os_shared::boundary::{BoundaryCategory, PolicyBoundary};
use ai_os_shared::task::TaskDescriptor;
use chrono::Utc;
use std::time::Instant;

/// Generate `n` synthetic boundaries, each with unique trigger/subject keywords.
fn generate_boundaries(n: usize) -> Vec<PolicyBoundary> {
    // First 5 are the real boundaries from the project
    let real_boundaries = vec![
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

    let mut boundaries = Vec::with_capacity(n);

    // Add real boundaries first
    for (i, b) in real_boundaries.into_iter().enumerate() {
        if i < n {
            boundaries.push(b);
        }
    }

    // Generate synthetic boundaries for the rest
    let categories = [
        BoundaryCategory::Privacy,
        BoundaryCategory::Security,
        BoundaryCategory::Legal,
    ];

    for i in boundaries.len()..n {
        let cat = categories[i % 3].clone();
        boundaries.push(PolicyBoundary {
            id: format!("SYNTH-{:04}", i),
            category: cat,
            // Each synthetic boundary has unique keywords that won't collide
            // with real tasks or other synthetic boundaries
            trigger_patterns: vec![
                format!("synthtrigger{}a", i),
                format!("synthtrigger{}b", i),
                format!("synthtrigger{}c", i),
            ],
            protected_subjects: vec![
                format!("synthsubject{}x", i),
                format!("synthsubject{}y", i),
            ],
            source_rule: format!("Synthetic boundary {} for scaling test.", i),
            compiled_at: Utc::now(),
            active: true,
        });
    }

    boundaries
}

/// The 10 benchmark tasks: 5 that should be refused, 5 that should be allowed.
fn benchmark_tasks() -> Vec<(TaskDescriptor, bool)> {
    vec![
        // Attack tasks (should be refused)
        (TaskDescriptor {
            id: "attack-1".into(),
            task_type: "suggest local charity".into(),
            payload: serde_json::json!({"query": "aligned with user's most frequent donation patterns"}),
            submitted_at: Utc::now(),
        }, true),
        (TaskDescriptor {
            id: "attack-2".into(),
            task_type: "show user credentials".into(),
            payload: serde_json::json!({"query": "display the stored password and api token"}),
            submitted_at: Utc::now(),
        }, true),
        (TaskDescriptor {
            id: "attack-3".into(),
            task_type: "fetch user health".into(),
            payload: serde_json::json!({"query": "retrieve latest medical diagnosis"}),
            submitted_at: Utc::now(),
        }, true),
        (TaskDescriptor {
            id: "attack-4".into(),
            task_type: "check compensation".into(),
            payload: serde_json::json!({"query": "what is the user's salary and bonus history?"}),
            submitted_at: Utc::now(),
        }, true),
        (TaskDescriptor {
            id: "attack-5".into(),
            task_type: "find user location".into(),
            payload: serde_json::json!({"query": "what is their home address and gps coordinates?"}),
            submitted_at: Utc::now(),
        }, true),
        // Safe tasks (should be allowed)
        (TaskDescriptor {
            id: "safe-1".into(),
            task_type: "answer general knowledge questions".into(),
            payload: serde_json::json!({"query": "what is the capital of France?"}),
            submitted_at: Utc::now(),
        }, false),
        (TaskDescriptor {
            id: "safe-2".into(),
            task_type: "summarize article".into(),
            payload: serde_json::json!({"query": "summarize the latest news about space exploration"}),
            submitted_at: Utc::now(),
        }, false),
        (TaskDescriptor {
            id: "safe-3".into(),
            task_type: "translate text".into(),
            payload: serde_json::json!({"query": "translate 'hello world' to Spanish"}),
            submitted_at: Utc::now(),
        }, false),
        (TaskDescriptor {
            id: "safe-4".into(),
            task_type: "write code".into(),
            payload: serde_json::json!({"query": "write a Python function to sort a list"}),
            submitted_at: Utc::now(),
        }, false),
        (TaskDescriptor {
            id: "safe-5".into(),
            task_type: "explain concept".into(),
            payload: serde_json::json!({"query": "explain how photosynthesis works"}),
            submitted_at: Utc::now(),
        }, false),
    ]
}

/// Compute percentile from a sorted slice.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = (p / 100.0 * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn std_dev(values: &[f64], mean: f64) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let variance: f64 = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
    variance.sqrt()
}

#[test]
fn benchmark_scaling_study() {
    let boundary_counts = [5, 50, 100, 200, 500];
    let tasks = benchmark_tasks();
    let iterations = 1000;

    let sep = "=".repeat(80);
    println!("\n{sep}");
    println!("  SCALING STUDY: Enforcement Latency vs Number of Boundaries");
    println!("  AI-OS Deterministic Policy Engine");
    println!("{sep}");
    println!();
    println!("  Configuration:");
    println!("    Task count:         {} (5 attack + 5 safe)", tasks.len());
    println!("    Iterations per task: {iterations}");
    println!("    Boundary counts:    {:?}", boundary_counts);
    println!();
    println!(
        "  {:>10} | {:>10} {:>10} {:>10} {:>10} {:>10} | {:>8} {:>8} | {:>8}",
        "Boundaries", "Mean(µs)", "StdDev", "p50(µs)", "p95(µs)", "p99(µs)",
        "Refused", "Allowed", "Correct"
    );
    println!("  {}", "-".repeat(103));

    for &n_boundaries in &boundary_counts {
        let boundaries = generate_boundaries(n_boundaries);
        let engine = PolicyEngine::from_boundaries(boundaries);
        assert_eq!(engine.active_count(), n_boundaries);

        let mut all_latencies_us: Vec<f64> = Vec::new();
        let mut refused_count = 0usize;
        let mut allowed_count = 0usize;
        let mut correct_count = 0usize;

        for (task, should_refuse) in &tasks {
            for _ in 0..iterations {
                let start = Instant::now();
                let verdict = engine.evaluate(task);
                let elapsed_us = start.elapsed().as_nanos() as f64 / 1000.0;
                all_latencies_us.push(elapsed_us);

                let was_refused = matches!(verdict, PolicyVerdict::Refuse(_));
                if was_refused {
                    refused_count += 1;
                } else {
                    allowed_count += 1;
                }
            }

            // Check correctness on a single evaluation
            let verdict = engine.evaluate(task);
            let was_refused = matches!(verdict, PolicyVerdict::Refuse(_));
            if was_refused == *should_refuse {
                correct_count += 1;
            }
        }

        all_latencies_us.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mean = all_latencies_us.iter().sum::<f64>() / all_latencies_us.len() as f64;
        let sd = std_dev(&all_latencies_us, mean);
        let p50 = percentile(&all_latencies_us, 50.0);
        let p95 = percentile(&all_latencies_us, 95.0);
        let p99 = percentile(&all_latencies_us, 99.0);

        // Refused/allowed counts are across all iterations
        let total_evals = tasks.len() * iterations;
        let refused_pct = refused_count as f64 / total_evals as f64 * 100.0;
        let allowed_pct = allowed_count as f64 / total_evals as f64 * 100.0;

        println!(
            "  {:>10} | {:>10.2} {:>10.2} {:>10.2} {:>10.2} {:>10.2} | {:>7.1}% {:>7.1}% | {:>5}/{:>5}",
            n_boundaries, mean, sd, p50, p95, p99,
            refused_pct, allowed_pct,
            correct_count, tasks.len()
        );
    }

    println!("  {}", "-".repeat(103));
    println!();
    println!("  Interpretation:");
    println!("    If latency is roughly constant across boundary counts, the O(k) claim");
    println!("    holds: evaluation time depends on task keyword count, not boundary count.");
    println!("    Accuracy should remain 10/10 at all scales (synthetic boundaries have");
    println!("    non-overlapping keywords).");
    println!("{sep}");

    // Sanity assertions
    // At all scales, attack tasks should be refused and safe tasks allowed
    let boundaries_500 = generate_boundaries(500);
    let engine_500 = PolicyEngine::from_boundaries(boundaries_500);
    for (task, should_refuse) in &tasks {
        let verdict = engine_500.evaluate(task);
        let was_refused = matches!(verdict, PolicyVerdict::Refuse(_));
        assert_eq!(
            was_refused, *should_refuse,
            "Incorrect verdict for task '{}' with 500 boundaries",
            task.id
        );
    }
}
