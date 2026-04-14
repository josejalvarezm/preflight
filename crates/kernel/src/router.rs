use ai_os_shared::error::{AiOsError, Result};
use ai_os_shared::task::TaskDescriptor;
use crate::roles::RoleRegistry;

/// The result of a routing decision.
#[derive(Debug, Clone)]
pub struct RoutingDecision {
    pub agent_id: String,
    pub rationale: String,
}

/// Route a task to the best-matching agent based on the role registry.
///
/// Fail-closed: if no agent matches, returns an error.
#[must_use = "routing decision must be checked for policy violations"]
pub fn route(registry: &RoleRegistry, task: &TaskDescriptor) -> Result<RoutingDecision> {
    let candidates = registry.find_by_task_type(&task.task_type);

    if candidates.is_empty() {
        return Err(AiOsError::NoAgentForTask(task.task_type.clone()));
    }

    let best = &candidates[0];

    Ok(RoutingDecision {
        agent_id: best.id.clone(),
        rationale: format!(
            "Task type '{}' matched agent '{}' via capability keyword overlap",
            task.task_type, best.id
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_os_shared::contract::*;
    use chrono::Utc;
    use std::collections::HashMap;

    fn test_registry() -> RoleRegistry {
        let mut agents = HashMap::new();
        agents.insert(
            "compiler".to_string(),
            AgentContract {
                id: "compiler".to_string(),
                version: 1,
                rules: vec!["Parse files".into()],
                constraints: vec!["Halt on error".into()],
                capabilities: vec!["Parse YAML files".into(), "Validate schema".into()],
            },
        );

        let manifest = ContractManifest {
            version: "1.0.0".into(),
            compiled_at: Utc::now(),
            global: GlobalContract {
                rules: vec![],
                constraints: vec![],
            },
            agents,
            boundaries: vec![],
        };

        RoleRegistry::from_manifest(&manifest)
    }

    #[test]
    fn routes_matching_task() {
        let registry = test_registry();
        let task = TaskDescriptor {
            id: "task-1".into(),
            task_type: "validate yaml schema".into(),
            payload: serde_json::Value::Null,
            submitted_at: Utc::now(),
        };

        let decision = route(&registry, &task).unwrap();
        assert_eq!(decision.agent_id, "compiler");
    }

    #[test]
    fn fails_on_unmatched_task() {
        let registry = test_registry();
        let task = TaskDescriptor {
            id: "task-2".into(),
            task_type: "deploy infrastructure".into(),
            payload: serde_json::Value::Null,
            submitted_at: Utc::now(),
        };

        let result = route(&registry, &task);
        assert!(result.is_err());
    }
}
