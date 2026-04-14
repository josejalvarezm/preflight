//! Limitation registry — append-only structured storage.

use ai_os_shared::error::{AiOsError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LimitationStatus {
    Open,
    Resolved,
    Verified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resolution {
    pub resolved_at: DateTime<Utc>,
    pub commit_sha: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Limitation {
    pub id: String,
    pub component: String,
    pub description: String,
    pub declared_at: DateTime<Utc>,
    pub status: LimitationStatus,
    pub commits: Vec<String>,
    pub resolution: Option<Resolution>,
}

/// The in-memory representation of the limitation registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitationRegistry {
    entries: Vec<Limitation>,
}

impl LimitationRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        LimitationRegistry {
            entries: Vec::new(),
        }
    }

    /// Load from a JSON file.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(AiOsError::Io)?;
        let registry: LimitationRegistry = serde_json::from_str(&content)?;
        Ok(registry)
    }

    /// Save to a JSON file. This is a full overwrite, but the data is append-only
    /// in the logical sense (no entries are ever removed from the Vec).
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(AiOsError::Io)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json).map_err(AiOsError::Io)?;
        Ok(())
    }

    /// Generate the next limitation ID (LIM-NNN).
    pub fn next_id(&self) -> String {
        format!("LIM-{:03}", self.entries.len() + 1)
    }

    /// Append a limitation. Never removes existing entries.
    pub fn append(&mut self, entry: Limitation) {
        self.entries.push(entry);
    }

    /// Get a mutable reference to a limitation by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Limitation> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    /// Get a reference to a limitation by ID.
    pub fn get(&self, id: &str) -> Option<&Limitation> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Get all entries.
    pub fn entries(&self) -> &[Limitation] {
        &self.entries
    }

    /// Count entries by status.
    pub fn count_by_status(&self, status: &LimitationStatus) -> usize {
        self.entries.iter().filter(|e| &e.status == status).count()
    }
}

impl Default for LimitationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_id_increments() {
        let mut reg = LimitationRegistry::new();
        assert_eq!(reg.next_id(), "LIM-001");

        reg.append(Limitation {
            id: "LIM-001".into(),
            component: "C1".into(),
            description: "test".into(),
            declared_at: Utc::now(),
            status: LimitationStatus::Open,
            commits: vec![],
            resolution: None,
        });

        assert_eq!(reg.next_id(), "LIM-002");
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("limitations.json");

        let mut reg = LimitationRegistry::new();
        reg.append(Limitation {
            id: "LIM-001".into(),
            component: "C2".into(),
            description: "Keyword-based only".into(),
            declared_at: Utc::now(),
            status: LimitationStatus::Open,
            commits: vec!["abc123".into()],
            resolution: None,
        });

        reg.save(&path).unwrap();

        let loaded = LimitationRegistry::load(&path).unwrap();
        assert_eq!(loaded.entries().len(), 1);
        assert_eq!(loaded.entries()[0].id, "LIM-001");
        assert_eq!(loaded.entries()[0].commits, vec!["abc123"]);
    }

    #[test]
    fn count_by_status() {
        let mut reg = LimitationRegistry::new();
        reg.append(Limitation {
            id: "LIM-001".into(),
            component: "C1".into(),
            description: "a".into(),
            declared_at: Utc::now(),
            status: LimitationStatus::Open,
            commits: vec![],
            resolution: None,
        });
        reg.append(Limitation {
            id: "LIM-002".into(),
            component: "C1".into(),
            description: "b".into(),
            declared_at: Utc::now(),
            status: LimitationStatus::Resolved,
            commits: vec![],
            resolution: None,
        });

        assert_eq!(reg.count_by_status(&LimitationStatus::Open), 1);
        assert_eq!(reg.count_by_status(&LimitationStatus::Resolved), 1);
        assert_eq!(reg.count_by_status(&LimitationStatus::Verified), 0);
    }
}
