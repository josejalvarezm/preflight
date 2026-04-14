use ai_os_shared::error::{AiOsError, Result};
use ai_os_shared::task::DecisionLogEntry;
use sha2::{Digest, Sha256};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

/// SHA-256 genesis hash: 64 hex zeros.
const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// Compute the SHA-256 hex digest of a JSON line.
fn compute_hash(json_line: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(json_line.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Recover the chain-head hash from the last non-empty line of an existing log file.
/// Returns the genesis hash if the file is empty or does not exist.
fn recover_last_hash(path: &Path) -> Result<String> {
    if !path.exists() {
        return Ok(GENESIS_HASH.to_string());
    }
    let content = std::fs::read_to_string(path).map_err(AiOsError::Io)?;
    match content.lines().rev().find(|l| !l.trim().is_empty()) {
        Some(line) => Ok(compute_hash(line)),
        None => Ok(GENESIS_HASH.to_string()),
    }
}

/// Append-only, hash-chained structured decision logger (JSON Lines format).
///
/// Each entry includes a `prev_hash` field containing the SHA-256 of
/// the previous entry's serialised JSON line. The first entry references
/// the genesis hash (64 zeros). This makes the log tamper-evident:
/// modifying or deleting any entry breaks the chain from that point forward.
pub struct DecisionLogger {
    path: PathBuf,
    /// SHA-256 of the last written JSON line (chain head).
    last_hash: String,
}

impl DecisionLogger {
    /// Create a new logger. Creates the file if it doesn't exist.
    /// Recovers the chain head from the last line of an existing file.
    pub fn new(path: &Path) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(AiOsError::Io)?;
        }

        // Create the file if it doesn't exist (append mode ensures we never truncate)
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(AiOsError::Io)?;

        let last_hash = recover_last_hash(path)?;

        Ok(DecisionLogger {
            path: path.to_path_buf(),
            last_hash,
        })
    }

    /// Append a decision log entry with hash-chaining. Append-only — never overwrites.
    ///
    /// Sets `entry.prev_hash` to the hash of the previous log line, serialises
    /// the entry as a JSON line, then updates the chain head.
    pub fn log(&mut self, entry: &DecisionLogEntry) -> Result<()> {
        // Build the chained entry
        let mut chained = entry.clone();
        chained.prev_hash = self.last_hash.clone();

        let mut line = serde_json::to_string(&chained)?;

        // Update chain head before writing (so even a partial write
        // doesn't corrupt the in-memory state on the next call).
        self.last_hash = compute_hash(&line);

        line.push('\n');

        let mut file = OpenOptions::new()
            .append(true)
            .open(&self.path)
            .map_err(AiOsError::Io)?;

        file.write_all(line.as_bytes()).map_err(AiOsError::Io)?;

        Ok(())
    }

    /// Read all log entries (for auditing/testing).
    pub fn read_all(&self) -> Result<Vec<DecisionLogEntry>> {
        let content = std::fs::read_to_string(&self.path).map_err(AiOsError::Io)?;
        let mut entries = Vec::new();

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let entry: DecisionLogEntry = serde_json::from_str(line)?;
            entries.push(entry);
        }

        Ok(entries)
    }

    /// Verify the hash chain of the entire log file.
    ///
    /// Returns `Ok(true)` if every entry's `prev_hash` matches the SHA-256
    /// of the preceding JSON line (or the genesis hash for the first entry).
    /// Returns `Ok(false)` if tampering is detected.
    pub fn verify_chain(path: &Path) -> Result<bool> {
        let content = std::fs::read_to_string(path).map_err(AiOsError::Io)?;
        let mut expected_prev = GENESIS_HASH.to_string();

        for line in content.lines().filter(|l| !l.trim().is_empty()) {
            let entry: DecisionLogEntry = serde_json::from_str(line)?;
            if entry.prev_hash != expected_prev {
                return Ok(false);
            }
            expected_prev = compute_hash(line);
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_os_shared::task::TaskStatus;
    use chrono::Utc;

    fn make_entry(task_id: &str) -> DecisionLogEntry {
        DecisionLogEntry {
            timestamp: Utc::now(),
            task_id: task_id.into(),
            selected_agent: "compiler".into(),
            rationale: "Matched by capability".into(),
            outcome: Some(TaskStatus::Success),
            prev_hash: String::new(),
        }
    }

    #[test]
    fn log_and_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("decisions.jsonl");

        let mut logger = DecisionLogger::new(&log_path).unwrap();

        logger.log(&make_entry("task-1")).unwrap();
        logger.log(&make_entry("task-1")).unwrap();

        let entries = logger.read_all().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].task_id, "task-1");
    }

    #[test]
    fn append_only_behaviour() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("decisions.jsonl");

        let mut logger1 = DecisionLogger::new(&log_path).unwrap();
        logger1.log(&make_entry("task-1")).unwrap();

        // Open a second logger on the same file — should append, not overwrite
        let mut logger2 = DecisionLogger::new(&log_path).unwrap();
        logger2.log(&make_entry("task-2")).unwrap();

        let entries = logger2.read_all().unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn hash_chain_is_valid() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("decisions.jsonl");

        let mut logger = DecisionLogger::new(&log_path).unwrap();
        logger.log(&make_entry("task-1")).unwrap();
        logger.log(&make_entry("task-2")).unwrap();
        logger.log(&make_entry("task-3")).unwrap();

        assert!(DecisionLogger::verify_chain(&log_path).unwrap());
    }

    #[test]
    fn first_entry_has_genesis_hash() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("decisions.jsonl");

        let mut logger = DecisionLogger::new(&log_path).unwrap();
        logger.log(&make_entry("task-1")).unwrap();

        let entries = logger.read_all().unwrap();
        assert_eq!(entries[0].prev_hash, GENESIS_HASH);
    }

    #[test]
    fn tampered_log_detected() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("decisions.jsonl");

        let mut logger = DecisionLogger::new(&log_path).unwrap();
        logger.log(&make_entry("task-1")).unwrap();
        logger.log(&make_entry("task-2")).unwrap();

        // Tamper: replace the first line with a modified entry
        let content = std::fs::read_to_string(&log_path).unwrap();
        let tampered = content.replacen("task-1", "task-HACKED", 1);
        std::fs::write(&log_path, tampered).unwrap();

        assert!(!DecisionLogger::verify_chain(&log_path).unwrap());
    }

    #[test]
    fn cross_instance_chain_continuity() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("decisions.jsonl");

        // First logger writes two entries
        let mut logger1 = DecisionLogger::new(&log_path).unwrap();
        logger1.log(&make_entry("task-1")).unwrap();
        logger1.log(&make_entry("task-2")).unwrap();

        // Second logger picks up the chain
        let mut logger2 = DecisionLogger::new(&log_path).unwrap();
        logger2.log(&make_entry("task-3")).unwrap();

        // Entire chain should be valid
        assert!(DecisionLogger::verify_chain(&log_path).unwrap());

        let entries = logger2.read_all().unwrap();
        assert_eq!(entries.len(), 3);
    }
}
