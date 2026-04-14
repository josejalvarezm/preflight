//! Resolver — tracks limitation resolution lifecycle.
//!
//! Status transitions: Open → Resolved → Verified.
//! Backward transitions are not permitted.

use crate::registry::{LimitationRegistry, LimitationStatus, Resolution};
use ai_os_shared::error::{AiOsError, Result};
use chrono::Utc;

/// Mark a limitation as resolved with a commit SHA and note.
pub fn resolve(
    registry: &mut LimitationRegistry,
    lim_id: &str,
    commit_sha: &str,
    note: &str,
) -> Result<()> {
    let entry = registry
        .get_mut(lim_id)
        .ok_or_else(|| AiOsError::Validation {
            file: "limitations.json".into(),
            message: format!("Limitation '{lim_id}' not found"),
        })?;

    if entry.status != LimitationStatus::Open {
        return Err(AiOsError::Validation {
            file: "limitations.json".into(),
            message: format!(
                "Cannot resolve '{lim_id}': status is {:?}, expected Open",
                entry.status
            ),
        });
    }

    entry.status = LimitationStatus::Resolved;
    entry.resolution = Some(Resolution {
        resolved_at: Utc::now(),
        commit_sha: commit_sha.to_string(),
        note: note.to_string(),
    });

    // Also link the resolution commit
    if !entry.commits.contains(&commit_sha.to_string()) {
        entry.commits.push(commit_sha.to_string());
    }

    Ok(())
}

/// Mark a resolved limitation as verified.
pub fn verify(registry: &mut LimitationRegistry, lim_id: &str) -> Result<()> {
    let entry = registry
        .get_mut(lim_id)
        .ok_or_else(|| AiOsError::Validation {
            file: "limitations.json".into(),
            message: format!("Limitation '{lim_id}' not found"),
        })?;

    if entry.status != LimitationStatus::Resolved {
        return Err(AiOsError::Validation {
            file: "limitations.json".into(),
            message: format!(
                "Cannot verify '{lim_id}': status is {:?}, expected Resolved",
                entry.status
            ),
        });
    }

    entry.status = LimitationStatus::Verified;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{Limitation, LimitationRegistry, LimitationStatus};
    use chrono::Utc;

    fn make_registry() -> LimitationRegistry {
        let mut reg = LimitationRegistry::new();
        reg.append(Limitation {
            id: "LIM-001".into(),
            component: "C1".into(),
            description: "test".into(),
            declared_at: Utc::now(),
            status: LimitationStatus::Open,
            commits: vec![],
            resolution: None,
        });
        reg
    }

    #[test]
    fn resolve_open_limitation() {
        let mut reg = make_registry();
        resolve(&mut reg, "LIM-001", "abc123", "Fixed the issue").unwrap();

        let lim = reg.get("LIM-001").unwrap();
        assert_eq!(lim.status, LimitationStatus::Resolved);
        assert!(lim.resolution.is_some());
        assert_eq!(lim.resolution.as_ref().unwrap().commit_sha, "abc123");
        assert!(lim.commits.contains(&"abc123".to_string()));
    }

    #[test]
    fn cannot_resolve_already_resolved() {
        let mut reg = make_registry();
        resolve(&mut reg, "LIM-001", "abc123", "Fixed").unwrap();
        let result = resolve(&mut reg, "LIM-001", "def456", "Fixed again");
        assert!(result.is_err());
    }

    #[test]
    fn verify_resolved_limitation() {
        let mut reg = make_registry();
        resolve(&mut reg, "LIM-001", "abc123", "Fixed").unwrap();
        verify(&mut reg, "LIM-001").unwrap();

        let lim = reg.get("LIM-001").unwrap();
        assert_eq!(lim.status, LimitationStatus::Verified);
    }

    #[test]
    fn cannot_verify_open_limitation() {
        let mut reg = make_registry();
        let result = verify(&mut reg, "LIM-001");
        assert!(result.is_err());
    }

    #[test]
    fn resolve_nonexistent_fails() {
        let mut reg = make_registry();
        let result = resolve(&mut reg, "LIM-999", "abc", "nope");
        assert!(result.is_err());
    }
}
