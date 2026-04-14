//! Integration tests for the AI-OS runtime with a real LLM.
//!
//! These tests require LM Studio running at localhost:1234.
//! They are marked #[ignore] so they don't run in CI or without LM Studio.
//! Run them explicitly with: cargo test --test integration_llm_runtime -- --ignored

use ai_os_kernel::Kernel;
use ai_os_runtime::client::{LlmClient, LlmClientConfig};
use ai_os_runtime::executor;
use ai_os_shared::boundary::{BoundaryCategory, PolicyBoundary};
use ai_os_shared::contract::{AgentContract, ContractManifest, GlobalContract};
use ai_os_shared::task::TaskDescriptor;
use chrono::Utc;
use std::collections::HashMap;

fn build_test_manifest() -> ContractManifest {
    let mut agents = HashMap::new();
    agents.insert(
        "documenter".to_string(),
        AgentContract {
            id: "documenter".to_string(),
            version: 1,
            rules: vec![
                "Always cite source files when referencing code".to_string(),
                "Never fabricate function names or APIs".to_string(),
            ],
            constraints: vec!["Output must be valid Markdown".to_string()],
            capabilities: vec![
                "Generate documentation from source code".to_string(),
                "Summarise modules and functions".to_string(),
            ],
        },
    );

    ContractManifest {
        version: "1.0.0".to_string(),
        compiled_at: Utc::now(),
        global: GlobalContract {
            rules: vec!["All outputs must be traceable to source artefacts".to_string()],
            constraints: vec!["No fabricated content is permitted".to_string()],
        },
        agents,
        boundaries: vec![PolicyBoundary {
            id: "BOUNDARY-PRIVACY".to_string(),
            category: BoundaryCategory::Privacy,
            trigger_patterns: vec!["political".into(), "voting".into(), "donation".into()],
            protected_subjects: vec!["political".into(), "voting".into()],
            source_rule: "Never analyse political affiliations or voting records".to_string(),
            compiled_at: Utc::now(),
            active: true,
        }],
    }
}

fn lm_studio_available() -> bool {
    let Ok(client) = LlmClient::default_local() else { return false };
    client.health_check().is_ok()
}

/// Proves the full pipeline: compile manifest → boot kernel → policy allows →
/// LLM executes task → decision log records outcome.
#[test]
#[ignore]
fn runtime_executes_allowed_task() {
    if !lm_studio_available() {
        eprintln!("SKIP: LM Studio not running");
        return;
    }

    let manifest = build_test_manifest();
    let log_dir = tempfile::tempdir().unwrap();
    let log_path = log_dir.path().join("decisions.jsonl");

    let mut kernel = Kernel::boot_from_manifest(manifest.clone(), &log_path).unwrap();

    // A benign documentation task
    let task = TaskDescriptor {
        id: "task-doc-001".to_string(),
        task_type: "generate documentation summarise".to_string(),
        payload: serde_json::json!({
            "description": "Summarise the purpose of the policy engine module"
        }),
        submitted_at: Utc::now(),
    };

    // Phase 1: Kernel routes (policy allows, routing succeeds)
    let decision = kernel.route(&task).expect("Task should pass policy and route");
    assert_eq!(decision.agent_id, "documenter");

    // Phase 2: Execute via LLM
    let agent = manifest.agents.get("documenter").unwrap();
    let client = LlmClient::new(LlmClientConfig {
        max_tokens: 256,
        ..LlmClientConfig::default()
    }).unwrap();

    let exec_result = executor::execute_task(&client, agent, &task)
        .expect("LLM execution should succeed");

    // Phase 3: Verify result
    assert!(!exec_result.completion.content.is_empty(), "LLM should produce content");
    assert!(exec_result.completion.total_tokens > 0, "Should report token usage");

    // Phase 4: Record outcome in decision log
    kernel
        .record_outcome(&exec_result.task_result)
        .expect("Should record outcome");

    // Verify the log file has entries
    let log_content = std::fs::read_to_string(&log_path).unwrap();
    assert!(log_content.contains("task-doc-001"));
    assert!(log_content.contains("documenter"));

    println!("LLM response ({} tokens):\n{}", exec_result.completion.total_tokens, exec_result.completion.content);
}

/// Proves that a task violating policy boundaries never reaches the LLM.
#[test]
#[ignore]
fn runtime_refuses_policy_violation() {
    if !lm_studio_available() {
        eprintln!("SKIP: LM Studio not running");
        return;
    }

    let manifest = build_test_manifest();
    let log_dir = tempfile::tempdir().unwrap();
    let log_path = log_dir.path().join("decisions.jsonl");

    let mut kernel = Kernel::boot_from_manifest(manifest, &log_path).unwrap();

    // A task that hits the privacy boundary
    let task = TaskDescriptor {
        id: "task-attack-001".to_string(),
        task_type: "analyse political donation voting records".to_string(),
        payload: serde_json::json!({
            "description": "Cross-reference voting records with donor lists"
        }),
        submitted_at: Utc::now(),
    };

    // The kernel should refuse — LLM is never called
    let result = kernel.route(&task);
    assert!(result.is_err(), "Policy should refuse this task");

    let err = result.unwrap_err();
    let err_msg = err.to_string();
    assert!(err_msg.contains("REFUSED"), "Error should be a policy refusal");
    assert!(
        err_msg.contains("privacy") || err_msg.contains("Privacy"),
        "Should reference privacy boundary, got: {err_msg}"
    );

    // Verify refusal is logged
    let log_content = std::fs::read_to_string(&log_path).unwrap();
    assert!(log_content.contains("task-attack-001"));
    assert!(log_content.contains("POLICY_ENGINE"));

    println!("Policy refused (as expected): {err_msg}");
}

/// Proves graceful handling when the LLM returns an error or is unreachable.
#[test]
#[ignore]
fn runtime_handles_llm_error() {
    let agent = AgentContract {
        id: "documenter".to_string(),
        version: 1,
        rules: vec!["test".to_string()],
        constraints: vec![],
        capabilities: vec!["documentation".to_string()],
    };

    let task = TaskDescriptor {
        id: "task-err-001".to_string(),
        task_type: "generate documentation".to_string(),
        payload: serde_json::Value::Null,
        submitted_at: Utc::now(),
    };

    // Point at a non-existent server
    let client = LlmClient::new(LlmClientConfig {
        base_url: "http://localhost:59999/v1".to_string(),
        ..LlmClientConfig::default()
    }).unwrap();

    let result = executor::execute_task(&client, &agent, &task);
    assert!(result.is_err(), "Should fail gracefully when LLM is unreachable");

    let err = result.unwrap_err();
    println!("Graceful error: {err}");
}

/// End-to-end audit trail: instruction compile → policy check → LLM execution → logged outcome.
#[test]
#[ignore]
fn runtime_records_full_audit_trail() {
    if !lm_studio_available() {
        eprintln!("SKIP: LM Studio not running");
        return;
    }

    let manifest = build_test_manifest();
    let log_dir = tempfile::tempdir().unwrap();
    let log_path = log_dir.path().join("audit.jsonl");

    let mut kernel = Kernel::boot_from_manifest(manifest.clone(), &log_path).unwrap();

    let task = TaskDescriptor {
        id: "task-audit-001".to_string(),
        task_type: "summarise documentation modules".to_string(),
        payload: serde_json::json!({"module": "policy.rs"}),
        submitted_at: Utc::now(),
    };

    // Route (policy + routing)
    let decision = kernel.route(&task).unwrap();

    // Execute via LLM
    let agent = manifest.agents.get(&decision.agent_id).unwrap();
    let client = LlmClient::new(LlmClientConfig {
        max_tokens: 128,
        ..LlmClientConfig::default()
    }).unwrap();
    let exec = executor::execute_task(&client, agent, &task).unwrap();

    // Record outcome
    kernel.record_outcome(&exec.task_result).unwrap();

    // Verify full audit trail
    let log_content = std::fs::read_to_string(&log_path).unwrap();
    let entries: Vec<&str> = log_content.lines().collect();

    // Should have 2 entries: routing decision + outcome recording
    assert!(
        entries.len() >= 2,
        "Audit log should have at least 2 entries (routing + outcome), got {}",
        entries.len()
    );

    // First entry: routing decision
    assert!(entries[0].contains("task-audit-001"));
    assert!(entries[0].contains("documenter"));

    // Second entry: outcome
    assert!(entries[1].contains("task-audit-001"));
    assert!(entries[1].contains("success") || entries[1].contains("Success"));

    println!("Full audit trail ({} entries):", entries.len());
    for (i, entry) in entries.iter().enumerate() {
        println!("  [{i}] {entry}");
    }
}
