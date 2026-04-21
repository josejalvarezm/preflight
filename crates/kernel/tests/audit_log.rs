//! Tamper-evidence tests for the hash-chained JSONL audit log.
//!
//! These tests operate at the **file level**: they write a valid chain via
//! `DecisionLogger`, then mutate the raw JSONL file to simulate attacker
//! tampering, and verify that `verify_chain()` detects every mutation.
//!
//! Run with: `cargo test --package ai-os-kernel --test audit_log`

use ai_os_kernel::logger::DecisionLogger;
use ai_os_shared::task::{DecisionLogEntry, TaskStatus};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::path::Path;

// ── helpers ──────────────────────────────────────────────────────────────

/// Create a `DecisionLogEntry` with a given task id and decision agent.
fn make_entry(task_id: &str, agent: &str) -> DecisionLogEntry {
    DecisionLogEntry {
        timestamp: Utc::now(),
        task_id: task_id.into(),
        selected_agent: agent.into(),
        rationale: "Matched by capability".into(),
        outcome: Some(TaskStatus::Success),
        prev_hash: String::new(), // logger fills this in
    }
}

/// Write `count` entries through the logger and return the log path.
fn write_valid_chain(dir: &Path, count: usize) -> std::path::PathBuf {
    let log_path = dir.join("decisions.jsonl");
    let mut logger = DecisionLogger::new(&log_path).unwrap();
    for i in 0..count {
        logger
            .log(&make_entry(&format!("task-{i}"), "compiler"))
            .unwrap();
    }
    log_path
}

/// Read raw JSONL lines from a file.
fn read_lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(String::from)
        .collect()
}

/// Overwrite the file with the given lines (each gets a trailing newline).
fn write_lines(path: &Path, lines: &[String]) {
    let content: String = lines.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(path, content).unwrap();
}

/// Compute the SHA-256 hex digest of a string (same algorithm as the logger).
fn sha256_hex(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    format!("{:x}", hasher.finalize())
}

// ── tests ────────────────────────────────────────────────────────────────

#[test]
fn valid_chain_verifies() {
    let dir = tempfile::tempdir().unwrap();
    let log_path = write_valid_chain(dir.path(), 5);

    assert!(DecisionLogger::verify_chain(&log_path).unwrap());
}

#[test]
fn altered_entry_detected() {
    let dir = tempfile::tempdir().unwrap();
    let log_path = write_valid_chain(dir.path(), 5);

    // Tamper: change the selected_agent in entry 2
    let mut lines = read_lines(&log_path);
    let mut entry: serde_json::Value = serde_json::from_str(&lines[2]).unwrap();
    entry["selected_agent"] = serde_json::Value::String("HACKED".into());
    lines[2] = serde_json::to_string(&entry).unwrap();
    write_lines(&log_path, &lines);

    assert!(!DecisionLogger::verify_chain(&log_path).unwrap());
}

#[test]
fn reordered_entries_detected() {
    let dir = tempfile::tempdir().unwrap();
    let log_path = write_valid_chain(dir.path(), 5);

    let mut lines = read_lines(&log_path);
    lines.swap(1, 3);
    write_lines(&log_path, &lines);

    assert!(!DecisionLogger::verify_chain(&log_path).unwrap());
}

#[test]
fn truncated_log_still_valid() {
    // Removing the *last* entry does not break any prev_hash link:
    // verify_chain walks forward and each remaining entry's prev_hash
    // still matches the hash of the entry before it.  This is by design;
    // detecting truncation requires an external expected-length check.
    let dir = tempfile::tempdir().unwrap();
    let log_path = write_valid_chain(dir.path(), 5);

    let mut lines = read_lines(&log_path);
    assert_eq!(lines.len(), 5);
    lines.pop(); // remove last entry
    write_lines(&log_path, &lines);

    // Chain of the remaining 4 entries is still internally consistent
    assert!(DecisionLogger::verify_chain(&log_path).unwrap());
}

#[test]
fn truncated_from_middle_detected() {
    // Removing an entry from the *middle* breaks the chain because the
    // entry after the gap has a prev_hash that no longer matches.
    let dir = tempfile::tempdir().unwrap();
    let log_path = write_valid_chain(dir.path(), 5);

    let mut lines = read_lines(&log_path);
    lines.remove(2); // remove entry at index 2
    write_lines(&log_path, &lines);

    assert!(!DecisionLogger::verify_chain(&log_path).unwrap());
}

#[test]
fn injected_entry_detected() {
    let dir = tempfile::tempdir().unwrap();
    let log_path = write_valid_chain(dir.path(), 5);

    let mut lines = read_lines(&log_path);

    // Craft an unsigned entry and inject it at position 2
    let fake = DecisionLogEntry {
        timestamp: Utc::now(),
        task_id: "INJECTED".into(),
        selected_agent: "evil-agent".into(),
        rationale: "Injected by attacker".into(),
        outcome: Some(TaskStatus::Refused),
        prev_hash: "0".repeat(64), // wrong hash
    };
    let fake_json = serde_json::to_string(&fake).unwrap();
    lines.insert(2, fake_json);
    write_lines(&log_path, &lines);

    assert!(!DecisionLogger::verify_chain(&log_path).unwrap());
}

#[test]
fn missing_prev_hash_detected() {
    let dir = tempfile::tempdir().unwrap();
    let log_path = write_valid_chain(dir.path(), 5);

    let mut lines = read_lines(&log_path);

    // Remove the prev_hash field from entry 3 via JSON manipulation.
    // serde will default it to the genesis hash on deserialization,
    // which won't match the expected hash of entry 2.
    let mut entry: serde_json::Value = serde_json::from_str(&lines[3]).unwrap();
    entry.as_object_mut().unwrap().remove("prev_hash");
    lines[3] = serde_json::to_string(&entry).unwrap();
    write_lines(&log_path, &lines);

    assert!(!DecisionLogger::verify_chain(&log_path).unwrap());
}

#[test]
fn wrong_prev_hash_detected() {
    let dir = tempfile::tempdir().unwrap();
    let log_path = write_valid_chain(dir.path(), 5);

    let mut lines = read_lines(&log_path);

    // Replace prev_hash of entry 3 with an arbitrary hash
    let mut entry: serde_json::Value = serde_json::from_str(&lines[3]).unwrap();
    entry["prev_hash"] =
        serde_json::Value::String(sha256_hex("totally-random-payload"));
    lines[3] = serde_json::to_string(&entry).unwrap();
    write_lines(&log_path, &lines);

    assert!(!DecisionLogger::verify_chain(&log_path).unwrap());
}

#[test]
fn genesis_hash_correct_for_first_entry() {
    let dir = tempfile::tempdir().unwrap();
    let log_path = write_valid_chain(dir.path(), 1);

    let lines = read_lines(&log_path);
    let entry: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    let prev = entry["prev_hash"].as_str().unwrap();

    // First entry must reference the genesis hash (64 zeros)
    assert_eq!(prev, "0".repeat(64));
}

#[test]
fn hash_chain_links_are_correct() {
    let dir = tempfile::tempdir().unwrap();
    let log_path = write_valid_chain(dir.path(), 5);

    let lines = read_lines(&log_path);

    // Manually verify each link
    for i in 1..lines.len() {
        let expected = sha256_hex(&lines[i - 1]);
        let entry: serde_json::Value = serde_json::from_str(&lines[i]).unwrap();
        let actual = entry["prev_hash"].as_str().unwrap();
        assert_eq!(
            actual, expected,
            "prev_hash mismatch at entry {i}: expected hash of line {prev}",
            prev = i - 1
        );
    }
}
