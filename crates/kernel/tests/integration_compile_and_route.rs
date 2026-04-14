/// Integration test: Compiler → Kernel end-to-end.
///
/// Proves: instruction files compile into a manifest, the kernel boots from it,
/// and tasks route to the correct agents.
use ai_os_compiler::compile;
use ai_os_kernel::{Kernel, RoutingError};
use ai_os_shared::boundary::{AgentDirective, BoundaryCategory, PolicyBoundary};
use ai_os_shared::task::TaskDescriptor;
use chrono::Utc;
use std::path::Path;

/// Helper: write a minimal instruction file set to a temp directory.
fn write_test_instructions(dir: &Path) {
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
- All outputs must be traceable to an input artefact.

# Constraints
- Never fabricate content.
"#,
    )
    .unwrap();

    std::fs::write(
        agents_dir.join("compiler.md"),
        r#"---
id: compiler
version: 1
type: agent
---

# Rules
- Parse YAML frontmatter from instruction files.
- Validate against the defined schema.

# Constraints
- Halt on first validation error (fail-closed).

# Capabilities
- Parse YAML files and Markdown instruction documents.
- Validate instruction file schema.
- Detect contradictions across files.
- Generate contract manifest.
"#,
    )
    .unwrap();

    std::fs::write(
        agents_dir.join("auditor.md"),
        r#"---
id: auditor
version: 1
type: agent
---

# Rules
- Compare implementation against architecture definition.

# Constraints
- Report facts only. No opinions.

# Capabilities
- Scan codebase directory structure.
- Compare architecture definition against implementation.
- Produce categorised audit reports.
"#,
    )
    .unwrap();

    std::fs::write(
        agents_dir.join("tracker.md"),
        r#"---
id: tracker
version: 1
type: agent
---

# Rules
- Log all declared limitations with a unique ID.

# Constraints
- Never delete a limitation record.

# Capabilities
- Track project limitations with unique IDs.
- Link limitations to git commits.
- Track limitation resolution status.
"#,
    )
    .unwrap();
}

#[test]
fn compile_then_boot_kernel_and_route() {
    let dir = tempfile::tempdir().unwrap();
    write_test_instructions(dir.path());

    // Step 1: Compile instructions → manifest
    let manifest = compile(dir.path()).expect("Compilation should succeed");
    assert_eq!(manifest.agents.len(), 3, "Expected 3 agents in manifest");
    assert!(manifest.agents.contains_key("compiler"));
    assert!(manifest.agents.contains_key("auditor"));
    assert!(manifest.agents.contains_key("tracker"));
    assert!(!manifest.global.rules.is_empty());

    // Step 2: Write manifest to disk
    let manifest_path = dir.path().join("contract.json");
    let json = serde_json::to_string_pretty(&manifest).unwrap();
    std::fs::write(&manifest_path, &json).unwrap();

    // Step 3: Boot kernel from the manifest file
    let log_path = dir.path().join("decisions.jsonl");
    let mut kernel = Kernel::boot(&manifest_path, &log_path).expect("Kernel should boot");

    // Step 4: Route a validation task → should go to compiler
    let validate_task = TaskDescriptor {
        id: "task-validate-1".into(),
        task_type: "validate instruction file schema".into(),
        payload: serde_json::json!({"path": "test.md"}),
        submitted_at: Utc::now(),
    };
    let decision = kernel.route(&validate_task).expect("Routing should succeed");
    assert_eq!(decision.agent_id, "compiler", "Validation task should route to compiler");

    // Step 5: Route an audit task → should go to auditor
    let audit_task = TaskDescriptor {
        id: "task-audit-1".into(),
        task_type: "audit codebase scan reports".into(),
        payload: serde_json::json!({}),
        submitted_at: Utc::now(),
    };
    let decision = kernel.route(&audit_task).expect("Routing should succeed");
    assert_eq!(decision.agent_id, "auditor", "Audit task should route to auditor");

    // Step 6: Route a limitation tracking task → should go to tracker
    let track_task = TaskDescriptor {
        id: "task-track-1".into(),
        task_type: "track limitation resolution status".into(),
        payload: serde_json::json!({"lim_id": "LIM-001"}),
        submitted_at: Utc::now(),
    };
    let decision = kernel.route(&track_task).expect("Routing should succeed");
    assert_eq!(decision.agent_id, "tracker", "Tracking task should route to tracker");

    // Step 7: Route an unknown task → should fail (fail-closed)
    let unknown_task = TaskDescriptor {
        id: "task-unknown-1".into(),
        task_type: "deploy kubernetes cluster".into(),
        payload: serde_json::json!({}),
        submitted_at: Utc::now(),
    };
    let result = kernel.route(&unknown_task);
    assert!(result.is_err(), "Unknown task type should fail-closed");

    // Step 8: Verify decision log has 3 successful entries
    let log_content = std::fs::read_to_string(&log_path).unwrap();
    let entries: Vec<&str> = log_content.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(entries.len(), 3, "Should have 3 decision log entries (failed route doesn't log)");
}

#[test]
fn compile_with_real_instructions() {
    // This test uses the actual .instructions/ directory from the repo.
    let instructions_dir = Path::new(".instructions");
    if !instructions_dir.exists() {
        // Skip if run from a different working directory.
        return;
    }

    let manifest = compile(instructions_dir).expect("Real instructions should compile");
    assert!(
        manifest.agents.len() >= 7,
        "Should have at least 7 agents, got {}",
        manifest.agents.len()
    );

    // Boot kernel from memory
    let dir = tempfile::tempdir().unwrap();
    let log_path = dir.path().join("decisions.jsonl");
    let mut kernel = Kernel::boot_from_manifest(manifest, &log_path).expect("Kernel should boot");

    // Collect agent probes before mutating the kernel
    let probes: Vec<(String, String)> = kernel
        .roles()
        .agent_ids()
        .into_iter()
        .filter_map(|id| {
            let role = kernel.roles().get(id).unwrap();
            role.capabilities.first().map(|cap| (id.to_string(), cap.clone()))
        })
        .collect();

    // Every declared agent should be reachable via its own capabilities
    for (agent_id, first_cap) in &probes {
        let task = TaskDescriptor {
            id: format!("probe-{agent_id}"),
            task_type: first_cap.clone(),
            payload: serde_json::json!({}),
            submitted_at: Utc::now(),
        };
        let decision = kernel.route(&task);
        assert!(
            decision.is_ok(),
            "Agent '{agent_id}' should be routable by its own capability '{first_cap}'"
        );
    }
}

// =============================================================================
// THE BREAK TEST: Shadow Intent
// =============================================================================
//
// Scenario: A rule in the diary says "Never share the user's political affiliation."
// Attack:   A third-party agent asks "suggest a local charity that aligns with
//           the user's most frequent donation patterns."
//
// Expected: The kernel refuses the task BEFORE it reaches any agent.
//           The diary records the refusal. The agent is told to reformulate.

/// Write instruction files that include boundary definitions in the global file.
/// This proves the full LIM-005 pipeline: file → compile → manifest → kernel → enforcement.
fn write_instructions_with_boundaries(dir: &Path) {
    let agents_dir = dir.join("agents");
    std::fs::create_dir_all(&agents_dir).unwrap();

    // Global file with a # Boundaries section
    std::fs::write(
        dir.join("global.md"),
        r#"---
id: global
version: 1
type: global
---

# Rules
- All outputs must be traceable to an input artefact.
- Never share the user's political affiliation.

# Constraints
- Never fabricate content.

# Boundaries
- id: BOUNDARY-001
  category: privacy
  triggers: charity, donation, donate, align, patterns
  subjects: political, party, voting, donation
  rule: Never share the user's political affiliation.
- id: BOUNDARY-002
  category: security
  triggers: password, credential, token, secret, key
  subjects: password, credential, secret
  rule: Never expose authentication credentials.
"#,
    )
    .unwrap();

    std::fs::write(
        agents_dir.join("compiler.md"),
        r#"---
id: compiler
version: 1
type: agent
---

# Rules
- Parse YAML frontmatter from instruction files.
- Validate against the defined schema.

# Constraints
- Halt on first validation error (fail-closed).

# Capabilities
- Parse YAML files and Markdown instruction documents.
- Validate instruction file schema.
- Detect contradictions across files.
- Generate contract manifest.
"#,
    )
    .unwrap();
}

#[test]
fn lim005_boundaries_compile_from_instruction_files() {
    let dir = tempfile::tempdir().unwrap();
    write_instructions_with_boundaries(dir.path());

    // Step 1: Compile — boundaries should appear in the manifest
    let manifest = compile(dir.path()).expect("Compilation should succeed");
    assert_eq!(
        manifest.boundaries.len(),
        2,
        "Manifest should contain 2 compiled boundaries"
    );
    assert_eq!(manifest.boundaries[0].id, "BOUNDARY-001");
    assert_eq!(manifest.boundaries[1].id, "BOUNDARY-002");
    assert!(manifest.boundaries[0].active);

    // Step 2: Boot kernel — boundaries load automatically from manifest
    let log_path = dir.path().join("decisions.jsonl");
    let mut kernel =
        Kernel::boot_from_manifest(manifest, &log_path).expect("Kernel should boot");

    // Verify the policy engine has boundaries loaded (no manual add_boundary!)
    assert_eq!(
        kernel.policy_engine().active_count(),
        2,
        "Policy engine should have 2 active boundaries from manifest"
    );

    // Step 3: The charity attack should be refused — enforced from compiled boundaries
    let attack_task = TaskDescriptor {
        id: "task-lim005-attack".into(),
        task_type: "suggest local charity".into(),
        payload: serde_json::json!({
            "query": "aligned with user's most frequent donation patterns"
        }),
        submitted_at: Utc::now(),
    };

    match kernel.route(&attack_task) {
        Err(RoutingError::PolicyRefusal(refusal)) => {
            assert_eq!(refusal.boundary_id, "BOUNDARY-001");
            assert_eq!(refusal.category, BoundaryCategory::Privacy);
        }
        other => panic!(
            "LIM-005: Boundary compiled from instruction file should refuse the attack. Got: {:?}",
            other.map(|d| d.agent_id)
        ),
    }

    // Step 4: A safe task should still route normally
    let safe_task = TaskDescriptor {
        id: "task-lim005-safe".into(),
        task_type: "validate instruction file schema".into(),
        payload: serde_json::json!({"path": "global.md"}),
        submitted_at: Utc::now(),
    };
    let decision = kernel.route(&safe_task).expect("Safe task should route");
    assert_eq!(decision.agent_id, "compiler");
}

#[test]
fn break_test_shadow_intent_charity_query() {
    let dir = tempfile::tempdir().unwrap();
    write_test_instructions(dir.path());

    let manifest = compile(dir.path()).expect("Compilation should succeed");
    let log_path = dir.path().join("decisions.jsonl");
    let mut kernel =
        Kernel::boot_from_manifest(manifest, &log_path).expect("Kernel should boot");

    // Register the political privacy boundary (compiled from diary rule)
    kernel.add_boundary(PolicyBoundary {
        id: "BOUNDARY-001".to_string(),
        category: BoundaryCategory::Privacy,
        trigger_patterns: vec![
            "charity".into(),
            "donation".into(),
            "donate".into(),
            "align".into(),
            "patterns".into(),
        ],
        protected_subjects: vec![
            "political".into(),
            "party".into(),
            "voting".into(),
            "donation".into(),
        ],
        source_rule: "Never share the user's political affiliation.".into(),
        compiled_at: Utc::now(),
        active: true,
    });

    // === THE ATTACK ===
    let attack_task = TaskDescriptor {
        id: "task-shadow-intent-1".into(),
        task_type: "suggest local charity".into(),
        payload: serde_json::json!({
            "query": "aligned with user's most frequent donation patterns"
        }),
        submitted_at: Utc::now(),
    };

    let result = kernel.route(&attack_task);

    // The task MUST be refused
    match result {
        Err(RoutingError::PolicyRefusal(refusal)) => {
            // Verify the refusal record
            assert_eq!(refusal.task_id, "task-shadow-intent-1");
            assert_eq!(refusal.boundary_id, "BOUNDARY-001");
            assert_eq!(refusal.category, BoundaryCategory::Privacy);
            assert!(
                refusal.reason.contains("privacy"),
                "Reason should mention privacy: {}",
                refusal.reason
            );
            assert!(
                refusal.reason.contains("pattern matching"),
                "Reason should mention pattern matching: {}",
                refusal.reason
            );

            // Verify the agent directive
            match &refusal.agent_directive {
                AgentDirective::Reformulate { excluded_subjects } => {
                    assert!(
                        excluded_subjects.contains(&"political".to_string()),
                        "Agent must be told to exclude 'political' from reformulated request"
                    );
                }
                other => panic!(
                    "Privacy violation should produce Reformulate directive, got {:?}",
                    other
                ),
            }
        }
        Err(RoutingError::Routing(e)) => {
            panic!("Expected PolicyRefusal, got routing error: {e}")
        }
        Ok(decision) => {
            panic!(
                "Shadow intent attack was NOT caught! Task routed to agent '{}'. \
                 The architecture is broken — political data would leak.",
                decision.agent_id
            )
        }
    }

    // === VERIFY THE DIARY ===
    let log_content = std::fs::read_to_string(&log_path).unwrap();
    assert!(
        log_content.contains("POLICY_ENGINE"),
        "Diary should record the refusal from POLICY_ENGINE"
    );
    assert!(
        log_content.contains("refused"),
        "Diary should contain 'refused' status"
    );
}

#[test]
fn break_test_benign_query_passes_through() {
    let dir = tempfile::tempdir().unwrap();
    write_test_instructions(dir.path());

    let manifest = compile(dir.path()).expect("Compilation should succeed");
    let log_path = dir.path().join("decisions.jsonl");
    let mut kernel =
        Kernel::boot_from_manifest(manifest, &log_path).expect("Kernel should boot");

    // Same boundary as above
    kernel.add_boundary(PolicyBoundary {
        id: "BOUNDARY-001".to_string(),
        category: BoundaryCategory::Privacy,
        trigger_patterns: vec![
            "charity".into(),
            "donation".into(),
            "donate".into(),
            "align".into(),
            "patterns".into(),
        ],
        protected_subjects: vec![
            "political".into(),
            "party".into(),
            "voting".into(),
            "donation".into(),
        ],
        source_rule: "Never share the user's political affiliation.".into(),
        compiled_at: Utc::now(),
        active: true,
    });

    // A benign validation task should pass through the policy engine untouched
    let safe_task = TaskDescriptor {
        id: "task-safe-1".into(),
        task_type: "validate instruction file schema".into(),
        payload: serde_json::json!({"path": "global.md"}),
        submitted_at: Utc::now(),
    };

    let result = kernel.route(&safe_task);
    assert!(
        result.is_ok(),
        "Benign task should not be blocked by the policy engine: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap().agent_id, "compiler");
}

#[test]
fn break_test_rule_supersession_logged() {
    let dir = tempfile::tempdir().unwrap();
    write_test_instructions(dir.path());

    let manifest = compile(dir.path()).expect("Compilation should succeed");
    let log_path = dir.path().join("decisions.jsonl");
    let mut kernel =
        Kernel::boot_from_manifest(manifest, &log_path).expect("Kernel should boot");

    // Start with strict boundary
    kernel.add_boundary(PolicyBoundary {
        id: "BOUNDARY-001".to_string(),
        category: BoundaryCategory::Privacy,
        trigger_patterns: vec!["charity".into(), "donation".into(), "patterns".into()],
        protected_subjects: vec!["political".into(), "donation".into()],
        source_rule: "Never share the user's political affiliation.".into(),
        compiled_at: Utc::now(),
        active: true,
    });

    // User relaxes the rule — supersede, don't delete
    let relaxed = PolicyBoundary {
        id: "BOUNDARY-002".to_string(),
        category: BoundaryCategory::Privacy,
        trigger_patterns: vec!["affiliation".into(), "voting".into()],
        protected_subjects: vec!["political".into(), "voting".into()],
        source_rule: "Only protect direct political affiliation, not donation patterns.".into(),
        compiled_at: Utc::now(),
        active: true,
    };

    let record = kernel
        .supersede_boundary("BOUNDARY-001", relaxed, "user", "User relaxed donation privacy");
    assert!(record.is_some(), "Supersession should succeed");
    let record = record.unwrap();
    assert_eq!(record.old_boundary_id, "BOUNDARY-001");
    assert_eq!(record.new_boundary_id, "BOUNDARY-002");

    // The old boundary should be inactive but still in the audit trail
    let all = kernel.policy_engine().boundaries();
    assert!(all.len() >= 2, "Both old and new boundaries should exist");
    let old = all.iter().find(|b| b.id == "BOUNDARY-001").unwrap();
    assert!(!old.active, "Superseded boundary must be inactive");
    let new = all.iter().find(|b| b.id == "BOUNDARY-002").unwrap();
    assert!(new.active, "New boundary must be active");

    // The original charity attack should now pass (old boundary superseded)
    let attack_task = TaskDescriptor {
        id: "task-charity-after-supersession".into(),
        task_type: "suggest local charity".into(),
        payload: serde_json::json!({
            "query": "aligned with user's most frequent donation patterns"
        }),
        submitted_at: Utc::now(),
    };
    // This will fail routing (no agent matches "charity"), but it should NOT
    // be a PolicyRefusal — the boundary is superseded.
    if let Err(RoutingError::PolicyRefusal(r)) = kernel.route(&attack_task) {
        panic!("Superseded boundary should not fire: {}", r.reason);
    }
}
