//! C5 — Limitation Tracker
//!
//! Append-only registry of known limitations. Links limitations to commits.
//! Tracks resolution status. Limitations are never silently deleted.

pub mod linker;
pub mod registry;
pub mod resolver;

use ai_os_shared::error::Result;
use std::path::Path;

/// High-level entry point: load, operate, and persist the limitation registry.
pub struct LimitationTracker {
    pub registry: registry::LimitationRegistry,
    path: std::path::PathBuf,
}

impl LimitationTracker {
    /// Open an existing registry file, or create a new empty one.
    pub fn open(path: &Path) -> Result<Self> {
        let registry = if path.exists() {
            registry::LimitationRegistry::load(path)?
        } else {
            registry::LimitationRegistry::new()
        };
        Ok(LimitationTracker {
            registry,
            path: path.to_path_buf(),
        })
    }

    /// Declare a new limitation. Returns the assigned ID.
    pub fn declare(
        &mut self,
        component: &str,
        description: &str,
    ) -> String {
        let id = self.registry.next_id();
        let entry = registry::Limitation {
            id: id.clone(),
            component: component.to_string(),
            description: description.to_string(),
            declared_at: chrono::Utc::now(),
            status: registry::LimitationStatus::Open,
            commits: vec![],
            resolution: None,
        };
        self.registry.append(entry);
        id
    }

    /// Link a limitation to a commit SHA.
    pub fn link_commit(&mut self, lim_id: &str, commit_sha: &str) -> Result<()> {
        linker::link(&mut self.registry, lim_id, commit_sha)
    }

    /// Resolve a limitation.
    pub fn resolve(
        &mut self,
        lim_id: &str,
        resolution_commit: &str,
        note: &str,
    ) -> Result<()> {
        resolver::resolve(&mut self.registry, lim_id, resolution_commit, note)
    }

    /// Save the registry to disk.
    pub fn save(&self) -> Result<()> {
        self.registry.save(&self.path)
    }

    /// List all limitations.
    pub fn list(&self) -> &[registry::Limitation] {
        self.registry.entries()
    }
}
