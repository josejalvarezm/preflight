use ai_os_shared::contract::ContractManifest;
use std::collections::HashMap;

/// A role definition derived from the contract manifest.
#[derive(Debug, Clone)]
pub struct AgentRole {
    pub id: String,
    pub capabilities: Vec<String>,
    pub constraints: Vec<String>,
    pub rules: Vec<String>,
}

/// Registry of all agent roles, built from the compiled manifest.
#[derive(Debug)]
pub struct RoleRegistry {
    roles: HashMap<String, AgentRole>,
    /// Maps capability keywords to agent IDs for routing.
    capability_index: HashMap<String, Vec<String>>,
}

impl RoleRegistry {
    /// Build the role registry from a compiled contract manifest.
    pub fn from_manifest(manifest: &ContractManifest) -> Self {
        let mut roles = HashMap::new();
        let mut capability_index: HashMap<String, Vec<String>> = HashMap::new();

        for (id, agent) in &manifest.agents {
            let role = AgentRole {
                id: id.clone(),
                capabilities: agent.capabilities.clone(),
                constraints: agent.constraints.clone(),
                rules: agent.rules.clone(),
            };

            // Index each capability keyword for routing
            for cap in &agent.capabilities {
                for word in extract_keywords(cap) {
                    capability_index
                        .entry(word)
                        .or_default()
                        .push(id.clone());
                }
            }

            roles.insert(id.clone(), role);
        }

        RoleRegistry {
            roles,
            capability_index,
        }
    }

    /// Look up an agent role by ID.
    pub fn get(&self, id: &str) -> Option<&AgentRole> {
        self.roles.get(id)
    }

    /// Find agents whose capabilities match the given task type.
    pub fn find_by_task_type(&self, task_type: &str) -> Vec<&AgentRole> {
        let keywords: Vec<String> = task_type
            .to_lowercase()
            .split_whitespace()
            .map(String::from)
            .collect();

        // Score agents by keyword overlap
        let mut scores: HashMap<&str, usize> = HashMap::new();
        for keyword in &keywords {
            if let Some(agent_ids) = self.capability_index.get(keyword) {
                for id in agent_ids {
                    *scores.entry(id.as_str()).or_insert(0) += 1;
                }
            }
        }

        // Return agents sorted by score (highest first)
        let mut scored: Vec<_> = scores.into_iter().collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1));

        scored
            .into_iter()
            .filter_map(|(id, _)| self.roles.get(id))
            .collect()
    }

    /// List all registered agent IDs.
    pub fn agent_ids(&self) -> Vec<&str> {
        self.roles.keys().map(|s| s.as_str()).collect()
    }
}

/// Extract lowercase keywords from a capability description.
fn extract_keywords(capability: &str) -> Vec<String> {
    capability
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2)
        .map(String::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_os_shared::contract::*;
    use chrono::Utc;

    fn test_manifest() -> ContractManifest {
        let mut agents = HashMap::new();
        agents.insert(
            "compiler".to_string(),
            AgentContract {
                id: "compiler".to_string(),
                version: 1,
                rules: vec!["Parse all files".into()],
                constraints: vec!["Reject invalid".into()],
                capabilities: vec![
                    "Parse YAML frontmatter from Markdown files".into(),
                    "Validate instruction file schema".into(),
                    "Generate contract.json manifest".into(),
                ],
            },
        );
        agents.insert(
            "auditor".to_string(),
            AgentContract {
                id: "auditor".to_string(),
                version: 1,
                rules: vec!["Compare architecture".into()],
                constraints: vec!["Report facts only".into()],
                capabilities: vec![
                    "Scan codebase directory structure".into(),
                    "Produce categorised audit reports".into(),
                ],
            },
        );

        ContractManifest {
            version: "1.0.0".to_string(),
            compiled_at: Utc::now(),
            global: GlobalContract {
                rules: vec!["Traceability".into()],
                constraints: vec!["No shared state".into()],
            },
            agents,
            boundaries: vec![],
        }
    }

    #[test]
    fn registry_finds_compiler_for_validate_task() {
        let manifest = test_manifest();
        let registry = RoleRegistry::from_manifest(&manifest);
        let matches = registry.find_by_task_type("validate instruction files");
        assert!(!matches.is_empty());
        assert_eq!(matches[0].id, "compiler");
    }

    #[test]
    fn registry_finds_auditor_for_audit_task() {
        let manifest = test_manifest();
        let registry = RoleRegistry::from_manifest(&manifest);
        let matches = registry.find_by_task_type("audit codebase scan");
        assert!(!matches.is_empty());
        assert_eq!(matches[0].id, "auditor");
    }
}
