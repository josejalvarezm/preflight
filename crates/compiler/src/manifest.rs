use ai_os_shared::boundary::PolicyBoundary;
use ai_os_shared::contract::{AgentContract, ContractManifest, GlobalContract};
use ai_os_shared::instruction::{InstructionFile, InstructionType};
use chrono::Utc;
use std::collections::HashMap;

/// Generate a `ContractManifest` from validated instruction files.
pub fn generate(files: &[InstructionFile]) -> ContractManifest {
    let mut global = GlobalContract {
        rules: Vec::new(),
        constraints: Vec::new(),
    };

    let mut agents: HashMap<String, AgentContract> = HashMap::new();
    let mut boundaries: Vec<PolicyBoundary> = Vec::new();

    let now = Utc::now();

    for file in files {
        match file.frontmatter.kind {
            InstructionType::Global => {
                global.rules.extend(file.rules.clone());
                global.constraints.extend(file.constraints.clone());
            }
            InstructionType::Agent => {
                agents.insert(
                    file.frontmatter.id.clone(),
                    AgentContract {
                        id: file.frontmatter.id.clone(),
                        version: file.frontmatter.version,
                        rules: file.rules.clone(),
                        constraints: file.constraints.clone(),
                        capabilities: file.capabilities.clone(),
                    },
                );
            }
        }

        // Compile boundary definitions from any file that has them
        for def in &file.boundaries {
            boundaries.push(PolicyBoundary {
                id: def.id.clone(),
                category: def.category.clone(),
                trigger_patterns: def.trigger_patterns.clone(),
                protected_subjects: def.protected_subjects.clone(),
                source_rule: def.source_rule.clone(),
                compiled_at: now,
                active: true,
            });
        }
    }

    ContractManifest {
        version: "1.0.0".to_string(),
        compiled_at: now,
        global,
        agents,
        boundaries,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_os_shared::boundary::BoundaryCategory;
    use ai_os_shared::instruction::{
        BoundaryDefinition, InstructionFile, InstructionFrontmatter, InstructionType,
    };

    #[test]
    fn generates_manifest_with_global_and_agents() {
        let global = InstructionFile {
            frontmatter: InstructionFrontmatter {
                id: "global".to_string(),
                version: 1,
                kind: InstructionType::Global,
            },
            source_path: "global.md".into(),
            rules: vec!["Global rule 1".into()],
            constraints: vec!["Global constraint 1".into()],
            capabilities: vec![],
            boundaries: vec![],
        };

        let agent = InstructionFile {
            frontmatter: InstructionFrontmatter {
                id: "compiler".to_string(),
                version: 1,
                kind: InstructionType::Agent,
            },
            source_path: "agents/compiler.md".into(),
            rules: vec!["Agent rule 1".into()],
            constraints: vec!["Agent constraint 1".into()],
            capabilities: vec!["Parse files".into()],
            boundaries: vec![],
        };

        let manifest = generate(&[global, agent]);

        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.global.rules.len(), 1);
        assert_eq!(manifest.global.constraints.len(), 1);
        assert!(manifest.agents.contains_key("compiler"));
        assert_eq!(manifest.agents["compiler"].capabilities.len(), 1);
        assert!(manifest.boundaries.is_empty());
    }

    #[test]
    fn compiles_boundary_definitions_into_manifest() {
        let global = InstructionFile {
            frontmatter: InstructionFrontmatter {
                id: "global".to_string(),
                version: 1,
                kind: InstructionType::Global,
            },
            source_path: "global.md".into(),
            rules: vec!["Protect privacy".into()],
            constraints: vec!["No PII sharing".into()],
            capabilities: vec![],
            boundaries: vec![
                BoundaryDefinition {
                    id: "BOUNDARY-001".to_string(),
                    category: BoundaryCategory::Privacy,
                    trigger_patterns: vec!["charity".into(), "donation".into()],
                    protected_subjects: vec!["political".into(), "donation".into()],
                    source_rule: "Never share political affiliation.".into(),
                },
                BoundaryDefinition {
                    id: "BOUNDARY-002".to_string(),
                    category: BoundaryCategory::Security,
                    trigger_patterns: vec!["password".into(), "token".into()],
                    protected_subjects: vec!["password".into(), "credential".into()],
                    source_rule: "Never expose credentials.".into(),
                },
            ],
        };

        let manifest = generate(&[global]);

        assert_eq!(manifest.boundaries.len(), 2);
        assert_eq!(manifest.boundaries[0].id, "BOUNDARY-001");
        assert_eq!(manifest.boundaries[0].category, BoundaryCategory::Privacy);
        assert!(manifest.boundaries[0].active);
        assert_eq!(manifest.boundaries[0].trigger_patterns, vec!["charity", "donation"]);
        assert_eq!(manifest.boundaries[1].id, "BOUNDARY-002");
        assert_eq!(manifest.boundaries[1].category, BoundaryCategory::Security);
    }
}
