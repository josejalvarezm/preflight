//! Linker — associates limitation IDs with git commit SHAs.

use crate::registry::LimitationRegistry;
use ai_os_shared::error::{AiOsError, Result};

/// Link a commit SHA to an existing limitation.
pub fn link(registry: &mut LimitationRegistry, lim_id: &str, commit_sha: &str) -> Result<()> {
    let entry = registry.get_mut(lim_id).ok_or_else(|| AiOsError::Validation {
        file: "limitations.json".into(),
        message: format!("Limitation '{lim_id}' not found"),
    })?;

    // Avoid duplicate links
    if !entry.commits.contains(&commit_sha.to_string()) {
        entry.commits.push(commit_sha.to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{Limitation, LimitationStatus};
    use chrono::Utc;

    fn make_registry() -> LimitationRegistry {
        let mut reg = LimitationRegistry::new();
        reg.append(Limitation {
            id: "LIM-001".into(),
            component: "C1".into(),
            description: "test limitation".into(),
            declared_at: Utc::now(),
            status: LimitationStatus::Open,
            commits: vec![],
            resolution: None,
        });
        reg
    }

    #[test]
    fn link_commit_to_limitation() {
        let mut reg = make_registry();
        link(&mut reg, "LIM-001", "abc123def").unwrap();
        assert_eq!(reg.get("LIM-001").unwrap().commits, vec!["abc123def"]);
    }

    #[test]
    fn duplicate_link_is_idempotent() {
        let mut reg = make_registry();
        link(&mut reg, "LIM-001", "abc123").unwrap();
        link(&mut reg, "LIM-001", "abc123").unwrap();
        assert_eq!(reg.get("LIM-001").unwrap().commits.len(), 1);
    }

    #[test]
    fn link_to_nonexistent_limitation_fails() {
        let mut reg = make_registry();
        let result = link(&mut reg, "LIM-999", "abc123");
        assert!(result.is_err());
    }
}
