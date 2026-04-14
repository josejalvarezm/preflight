use ai_os_shared::error::{AiOsError, Result};
use ai_os_shared::instruction::{InstructionFile, InstructionType};

/// Validate a parsed instruction file against the schema.
/// Returns `Ok(())` if valid, or an error describing the violation.
pub fn validate(file: &InstructionFile) -> Result<()> {
    let src = &file.source_path;
    let fm = &file.frontmatter;

    // ID must be non-empty and lowercase alphanumeric + hyphens
    if fm.id.is_empty() {
        return Err(AiOsError::Validation {
            file: src.clone(),
            message: "Frontmatter 'id' must not be empty".into(),
        });
    }

    if !fm
        .id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(AiOsError::Validation {
            file: src.clone(),
            message: format!(
                "Frontmatter 'id' must be lowercase alphanumeric + hyphens, got '{}'",
                fm.id
            ),
        });
    }

    // Version must be > 0
    if fm.version == 0 {
        return Err(AiOsError::Validation {
            file: src.clone(),
            message: "Frontmatter 'version' must be >= 1".into(),
        });
    }

    // Rules section is required for all types
    if file.rules.is_empty() {
        return Err(AiOsError::Validation {
            file: src.clone(),
            message: "At least one rule is required in the '# Rules' section".into(),
        });
    }

    // Constraints section is required for all types
    if file.constraints.is_empty() {
        return Err(AiOsError::Validation {
            file: src.clone(),
            message: "At least one constraint is required in the '# Constraints' section".into(),
        });
    }

    // Agent-type files must declare at least one capability
    if fm.kind == InstructionType::Agent && file.capabilities.is_empty() {
        return Err(AiOsError::Validation {
            file: src.clone(),
            message: "Agent instruction files must declare at least one capability".into(),
        });
    }

    // Global-type files should not declare capabilities
    if fm.kind == InstructionType::Global && !file.capabilities.is_empty() {
        return Err(AiOsError::Validation {
            file: src.clone(),
            message: "Global instruction files must not declare capabilities".into(),
        });
    }

    // Agent-type files should not declare boundaries
    if fm.kind == InstructionType::Agent && !file.boundaries.is_empty() {
        return Err(AiOsError::Validation {
            file: src.clone(),
            message: "Agent instruction files must not declare boundaries (use global files)".into(),
        });
    }

    // Validate boundary definitions
    for (i, boundary) in file.boundaries.iter().enumerate() {
        if boundary.id.is_empty() {
            return Err(AiOsError::Validation {
                file: src.clone(),
                message: format!("Boundary #{} has an empty id", i + 1),
            });
        }
        if boundary.trigger_patterns.is_empty() {
            return Err(AiOsError::Validation {
                file: src.clone(),
                message: format!("Boundary '{}' must have at least one trigger pattern", boundary.id),
            });
        }
        if boundary.protected_subjects.is_empty() {
            return Err(AiOsError::Validation {
                file: src.clone(),
                message: format!("Boundary '{}' must have at least one protected subject", boundary.id),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_os_shared::boundary::BoundaryCategory;
    use ai_os_shared::instruction::{
        BoundaryDefinition, InstructionFile, InstructionFrontmatter, InstructionType,
    };

    fn make_agent(id: &str, rules: Vec<&str>, constraints: Vec<&str>, caps: Vec<&str>) -> InstructionFile {
        InstructionFile {
            frontmatter: InstructionFrontmatter {
                id: id.to_string(),
                version: 1,
                kind: InstructionType::Agent,
            },
            source_path: format!("test/{id}.md"),
            rules: rules.into_iter().map(String::from).collect(),
            constraints: constraints.into_iter().map(String::from).collect(),
            capabilities: caps.into_iter().map(String::from).collect(),
            boundaries: vec![],
        }
    }

    #[test]
    fn valid_agent_passes() {
        let file = make_agent("compiler", vec!["rule1"], vec!["c1"], vec!["cap1"]);
        assert!(validate(&file).is_ok());
    }

    #[test]
    fn empty_id_rejected() {
        let file = make_agent("", vec!["rule1"], vec!["c1"], vec!["cap1"]);
        assert!(validate(&file).is_err());
    }

    #[test]
    fn uppercase_id_rejected() {
        let file = make_agent("BadId", vec!["rule1"], vec!["c1"], vec!["cap1"]);
        assert!(validate(&file).is_err());
    }

    #[test]
    fn agent_without_capabilities_rejected() {
        let file = make_agent("test", vec!["rule1"], vec!["c1"], vec![]);
        assert!(validate(&file).is_err());
    }

    #[test]
    fn no_rules_rejected() {
        let file = make_agent("test", vec![], vec!["c1"], vec!["cap1"]);
        assert!(validate(&file).is_err());
    }

    #[test]
    fn agent_with_boundaries_rejected() {
        let mut file = make_agent("test", vec!["rule1"], vec!["c1"], vec!["cap1"]);
        file.boundaries = vec![BoundaryDefinition {
            id: "B-001".into(),
            category: BoundaryCategory::Privacy,
            trigger_patterns: vec!["test".into()],
            protected_subjects: vec!["test".into()],
            source_rule: "Test rule".into(),
        }];
        assert!(validate(&file).is_err());
    }

    #[test]
    fn boundary_with_empty_triggers_rejected() {
        let file = InstructionFile {
            frontmatter: InstructionFrontmatter {
                id: "global".into(),
                version: 1,
                kind: InstructionType::Global,
            },
            source_path: "global.md".into(),
            rules: vec!["rule1".into()],
            constraints: vec!["c1".into()],
            capabilities: vec![],
            boundaries: vec![BoundaryDefinition {
                id: "B-001".into(),
                category: BoundaryCategory::Privacy,
                trigger_patterns: vec![],
                protected_subjects: vec!["test".into()],
                source_rule: "Test".into(),
            }],
        };
        assert!(validate(&file).is_err());
    }
}
