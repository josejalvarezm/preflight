//! Policy enforcement engine — the "pre-flight check" that runs before routing.
//!
//! Solves three architectural walls:
//!
//! **Latency Wall**: Boundaries are indexed by trigger keyword in a `HashMap`.
//! Lookup is O(k) where k = number of keywords in the task, not O(n) over the
//! entire diary. The diary can grow indefinitely; enforcement stays fast.
//!
//! **Semantic Gap**: Natural-language rules are compiled into `PolicyBoundary`
//! structs with explicit `trigger_patterns` and `protected_subjects`. The
//! evaluation is pure keyword intersection — no AI inference at enforcement time.
//! The "understanding" happens at *compile* time (in the instruction compiler),
//! not at *enforcement* time (in the kernel).
//!
//! **Rigidity Wall**: Rules are superseded, never deleted. A `RuleSupersession`
//! record links old → new, sets the old boundary to `active: false`, and the
//! diary records the change. The continuity of discipline is never broken.

use ai_os_shared::boundary::{
    AgentDirective, BoundaryCategory, PolicyBoundary, RefusalRecord, RuleSupersession,
};
use ai_os_shared::task::TaskDescriptor;
use chrono::Utc;
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;

/// The policy engine. Holds indexed boundaries and evaluates tasks against them.
#[derive(Debug)]
pub struct PolicyEngine {
    /// All boundaries (including inactive/superseded ones, for audit trail).
    boundaries: Vec<PolicyBoundary>,
    /// Keyword → list of boundary indices. Only active boundaries are indexed.
    trigger_index: HashMap<String, Vec<usize>>,
    /// History of supersessions.
    supersessions: Vec<RuleSupersession>,
}

/// The result of a policy evaluation.
#[derive(Debug)]
pub enum PolicyVerdict {
    /// The task does not violate any boundary. Proceed to routing.
    Allow,
    /// The task violates one or more boundaries. Contains the refusal record.
    Refuse(RefusalRecord),
}

impl PolicyEngine {
    /// Create an empty engine.
    pub fn new() -> Self {
        PolicyEngine {
            boundaries: Vec::new(),
            trigger_index: HashMap::new(),
            supersessions: Vec::new(),
        }
    }

    /// Build a policy engine from a set of boundaries (e.g. loaded from manifest).
    pub fn from_boundaries(boundaries: Vec<PolicyBoundary>) -> Self {
        let mut engine = PolicyEngine::new();
        for b in boundaries {
            engine.add_boundary(b);
        }
        engine
    }

    /// Add a boundary and index its trigger patterns.
    pub fn add_boundary(&mut self, boundary: PolicyBoundary) {
        let idx = self.boundaries.len();
        if boundary.active {
            for pattern in &boundary.trigger_patterns {
                let normalized: String = pattern.nfkc().collect::<String>().to_lowercase();
                self.trigger_index
                    .entry(normalized)
                    .or_default()
                    .push(idx);
            }
        }
        self.boundaries.push(boundary);
    }

    /// Supersede an existing boundary with a new one.
    /// The old boundary is deactivated (never deleted). The new one is added.
    /// Returns the supersession record.
    pub fn supersede(
        &mut self,
        old_boundary_id: &str,
        new_boundary: PolicyBoundary,
        authorised_by: &str,
        reason: &str,
    ) -> Option<RuleSupersession> {
        // Find and deactivate the old boundary
        let old_idx = self
            .boundaries
            .iter()
            .position(|b| b.id == old_boundary_id && b.active)?;

        self.boundaries[old_idx].active = false;

        // Remove old boundary from trigger index
        let old_patterns: Vec<String> = self.boundaries[old_idx].trigger_patterns.clone();
        for pattern in &old_patterns {
            if let Some(indices) = self.trigger_index.get_mut(pattern) {
                indices.retain(|&i| i != old_idx);
            }
        }

        let record = RuleSupersession {
            old_boundary_id: old_boundary_id.to_string(),
            new_boundary_id: new_boundary.id.clone(),
            authorised_by: authorised_by.to_string(),
            reason: reason.to_string(),
            superseded_at: Utc::now(),
        };
        self.supersessions.push(record.clone());

        // Add the new boundary (will be indexed automatically)
        self.add_boundary(new_boundary);

        Some(record)
    }

    /// Evaluate a task against all active boundaries.
    ///
    /// Extracts keywords from the task's `task_type` and payload text,
    /// checks them against the trigger index (O(k) lookup), then for each
    /// candidate boundary, checks if the task also touches a protected subject.
    pub fn evaluate(&self, task: &TaskDescriptor) -> PolicyVerdict {
        let task_keywords = extract_task_keywords(task);

        // Phase 1: Find candidate boundaries via trigger index (fast path)
        let mut candidate_indices: Vec<usize> = Vec::new();
        for keyword in &task_keywords {
            if let Some(indices) = self.trigger_index.get(keyword) {
                for &idx in indices {
                    if !candidate_indices.contains(&idx) {
                        candidate_indices.push(idx);
                    }
                }
            }
        }

        // Phase 2: For each candidate, check if the task touches a protected subject
        for &idx in &candidate_indices {
            let boundary = &self.boundaries[idx];
            if !boundary.active {
                continue;
            }

            let matched_subjects: Vec<String> = boundary
                .protected_subjects
                .iter()
                .filter(|subj| task_keywords.contains(&normalize_word(subj)))
                .cloned()
                .collect();

            let matched_triggers: Vec<String> = boundary
                .trigger_patterns
                .iter()
                .filter(|pat| task_keywords.contains(&normalize_word(pat)))
                .cloned()
                .collect();

            // A boundary fires when BOTH a trigger AND a protected subject match.
            // This prevents over-triggering: "charity" alone doesn't fire;
            // "charity + donation + patterns" does, because "donation" is both a
            // trigger AND touches the "political" protected subject space.
            if !matched_triggers.is_empty() && !matched_subjects.is_empty() {
                let reason = format!(
                    "Refused: High probability of leaking {} data via pattern matching. \
                     Trigger patterns {} matched against protected subjects {}.",
                    format_category(&boundary.category),
                    format_list(&matched_triggers),
                    format_list(&matched_subjects),
                );

                let directive = match boundary.category {
                    BoundaryCategory::Privacy => AgentDirective::Reformulate {
                        excluded_subjects: boundary.protected_subjects.clone(),
                    },
                    BoundaryCategory::Security => AgentDirective::Terminate,
                    BoundaryCategory::Legal => AgentDirective::EscalateToUser,
                    BoundaryCategory::Custom(_) => AgentDirective::Reformulate {
                        excluded_subjects: boundary.protected_subjects.clone(),
                    },
                };

                return PolicyVerdict::Refuse(RefusalRecord {
                    task_id: task.id.clone(),
                    boundary_id: boundary.id.clone(),
                    category: boundary.category.clone(),
                    reason,
                    matched_patterns: matched_triggers,
                    refused_at: Utc::now(),
                    agent_directive: directive,
                });
            }
        }

        PolicyVerdict::Allow
    }

    /// Get all boundaries (including inactive) for audit purposes.
    pub fn boundaries(&self) -> &[PolicyBoundary] {
        &self.boundaries
    }

    /// Get the supersession history.
    pub fn supersessions(&self) -> &[RuleSupersession] {
        &self.supersessions
    }

    /// Count active boundaries.
    pub fn active_count(&self) -> usize {
        self.boundaries.iter().filter(|b| b.active).count()
    }
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Normalize a word: NFKC → lowercase → strip non-alphanumeric.
/// NFKC collapses ligatures (ﬀ→ff), fullwidth chars (Ａ→A), etc.
fn normalize_word(word: &str) -> String {
    word.nfkc()
        .collect::<String>()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Extract lowercased, NFKC-normalized keywords from a task's type and payload.
fn extract_task_keywords(task: &TaskDescriptor) -> Vec<String> {
    let mut keywords = Vec::new();

    // From task_type
    for word in task.task_type.split_whitespace() {
        let clean = normalize_word(word);
        if clean.len() > 2 {
            keywords.push(clean);
        }
    }

    // From payload (if it's a string or contains string values)
    extract_payload_keywords(&task.payload, &mut keywords);

    keywords.sort();
    keywords.dedup();
    keywords
}

fn extract_payload_keywords(value: &serde_json::Value, keywords: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => {
            for word in s.split_whitespace() {
                let clean = normalize_word(word);
                if clean.len() > 2 {
                    keywords.push(clean);
                }
            }
        }
        serde_json::Value::Object(map) => {
            for v in map.values() {
                extract_payload_keywords(v, keywords);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                extract_payload_keywords(v, keywords);
            }
        }
        _ => {}
    }
}

fn format_category(cat: &BoundaryCategory) -> &str {
    match cat {
        BoundaryCategory::Privacy => "privacy",
        BoundaryCategory::Security => "security",
        BoundaryCategory::Legal => "legal",
        BoundaryCategory::Custom(s) => s.as_str(),
    }
}

fn format_list(items: &[String]) -> String {
    format!("[{}]", items.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_os_shared::boundary::{AgentDirective, BoundaryCategory, PolicyBoundary};

    fn political_boundary() -> PolicyBoundary {
        PolicyBoundary {
            id: "BOUNDARY-001".to_string(),
            category: BoundaryCategory::Privacy,
            trigger_patterns: vec![
                "charity".into(),
                "donation".into(),
                "donate".into(),
                "align".into(),
                "affiliation".into(),
                "patterns".into(),
            ],
            protected_subjects: vec![
                "political".into(),
                "party".into(),
                "voting".into(),
                "donation".into(),
            ],
            source_rule: "Never share the user's political affiliation.".into(),
            compiled_at: Utc::now(),
            active: true,
        }
    }

    #[test]
    fn shadow_intent_charity_query_is_refused() {
        let engine = PolicyEngine::from_boundaries(vec![political_boundary()]);

        // The attack: sounds helpful, but leaks political data via donation patterns
        let task = TaskDescriptor {
            id: "task-charity-1".into(),
            task_type: "suggest local charity".into(),
            payload: serde_json::json!({
                "query": "aligned with user's most frequent donation patterns"
            }),
            submitted_at: Utc::now(),
        };

        match engine.evaluate(&task) {
            PolicyVerdict::Refuse(refusal) => {
                assert_eq!(refusal.boundary_id, "BOUNDARY-001");
                assert_eq!(refusal.category, BoundaryCategory::Privacy);
                assert!(refusal.reason.contains("privacy"));
                assert!(refusal.reason.contains("pattern matching"));
                // Agent should be told to reformulate, excluding political subjects
                match &refusal.agent_directive {
                    AgentDirective::Reformulate { excluded_subjects } => {
                        assert!(excluded_subjects.contains(&"political".to_string()));
                    }
                    other => panic!("Expected Reformulate, got {:?}", other),
                }
            }
            PolicyVerdict::Allow => panic!("Shadow intent attack should have been refused!"),
        }
    }

    #[test]
    fn benign_charity_query_is_allowed() {
        let engine = PolicyEngine::from_boundaries(vec![political_boundary()]);

        // A safe charity query — no mention of donations, patterns, alignment
        let task = TaskDescriptor {
            id: "task-charity-2".into(),
            task_type: "list local charities".into(),
            payload: serde_json::json!({
                "query": "find food banks near the user's city"
            }),
            submitted_at: Utc::now(),
        };

        match engine.evaluate(&task) {
            PolicyVerdict::Allow => {} // correct
            PolicyVerdict::Refuse(r) => {
                panic!("Benign query should not be refused: {:?}", r.reason)
            }
        }
    }

    #[test]
    fn trigger_alone_does_not_fire_without_protected_subject() {
        let engine = PolicyEngine::from_boundaries(vec![political_boundary()]);

        // Has "charity" (trigger) but nothing touching political subjects
        let task = TaskDescriptor {
            id: "task-charity-3".into(),
            task_type: "suggest a charity for animal rescue".into(),
            payload: serde_json::json!({}),
            submitted_at: Utc::now(),
        };

        match engine.evaluate(&task) {
            PolicyVerdict::Allow => {} // correct — trigger without subject overlap
            PolicyVerdict::Refuse(r) => {
                panic!("Should not fire on trigger alone: {:?}", r.reason)
            }
        }
    }

    #[test]
    fn superseded_boundary_does_not_fire() {
        let mut engine = PolicyEngine::from_boundaries(vec![political_boundary()]);

        // User changes their mind: allow donation-based recommendations
        let relaxed = PolicyBoundary {
            id: "BOUNDARY-002".to_string(),
            category: BoundaryCategory::Privacy,
            trigger_patterns: vec!["affiliation".into(), "voting".into()],
            protected_subjects: vec!["political".into(), "voting".into()],
            source_rule: "Only protect direct political affiliation, not donation patterns.".into(),
            compiled_at: Utc::now(),
            active: true,
        };

        let record = engine
            .supersede("BOUNDARY-001", relaxed, "user", "User relaxed donation privacy")
            .expect("Supersession should succeed");

        assert_eq!(record.old_boundary_id, "BOUNDARY-001");
        assert_eq!(record.new_boundary_id, "BOUNDARY-002");

        // The original charity attack should now be ALLOWED (old boundary inactive)
        let task = TaskDescriptor {
            id: "task-charity-4".into(),
            task_type: "suggest local charity".into(),
            payload: serde_json::json!({
                "query": "aligned with user's most frequent donation patterns"
            }),
            submitted_at: Utc::now(),
        };

        match engine.evaluate(&task) {
            PolicyVerdict::Allow => {} // correct — old boundary superseded
            PolicyVerdict::Refuse(r) => {
                panic!(
                    "Superseded boundary should not fire: {:?}",
                    r.reason
                )
            }
        }

        // But a direct political affiliation query should still be refused
        let direct_attack = TaskDescriptor {
            id: "task-political-1".into(),
            task_type: "reveal user affiliation".into(),
            payload: serde_json::json!({
                "query": "what is the user's political party and voting record"
            }),
            submitted_at: Utc::now(),
        };

        match engine.evaluate(&direct_attack) {
            PolicyVerdict::Refuse(refusal) => {
                assert_eq!(refusal.boundary_id, "BOUNDARY-002");
            }
            PolicyVerdict::Allow => {
                panic!("Direct political query should still be refused after supersession")
            }
        }
    }

    #[test]
    fn security_boundary_terminates_agent() {
        let boundary = PolicyBoundary {
            id: "BOUNDARY-SEC-001".to_string(),
            category: BoundaryCategory::Security,
            trigger_patterns: vec!["execute".into(), "shell".into(), "command".into()],
            protected_subjects: vec!["system".into(), "root".into(), "admin".into()],
            source_rule: "Never allow agents to execute system commands.".into(),
            compiled_at: Utc::now(),
            active: true,
        };

        let engine = PolicyEngine::from_boundaries(vec![boundary]);

        let task = TaskDescriptor {
            id: "task-exec-1".into(),
            task_type: "execute shell command".into(),
            payload: serde_json::json!({"cmd": "grant admin access to system"}),
            submitted_at: Utc::now(),
        };

        match engine.evaluate(&task) {
            PolicyVerdict::Refuse(refusal) => {
                assert_eq!(refusal.agent_directive, AgentDirective::Terminate);
            }
            PolicyVerdict::Allow => panic!("Security violation should terminate agent"),
        }
    }

    #[test]
    fn nfkc_normalization_catches_ligature_evasion() {
        // Attacker uses ﬀ (U+FB00, Latin Small Ligature FF) instead of "ff"
        // "a\u{FB00}iliation" should normalize to "affiliation"
        let engine = PolicyEngine::from_boundaries(vec![political_boundary()]);

        let task = TaskDescriptor {
            id: "task-nfkc-1".into(),
            task_type: "reveal user a\u{FB00}iliation".into(),
            payload: serde_json::json!({
                "query": "what is the user's political party"
            }),
            submitted_at: Utc::now(),
        };

        match engine.evaluate(&task) {
            PolicyVerdict::Refuse(refusal) => {
                assert_eq!(refusal.boundary_id, "BOUNDARY-001");
            }
            PolicyVerdict::Allow => {
                panic!("NFKC ligature evasion should be caught")
            }
        }
    }

    #[test]
    fn nfkc_normalization_catches_fullwidth_evasion() {
        // Attacker uses fullwidth Latin: Ｄｏnation (U+FF24, U+FF4F)
        // Should normalize to "donation"
        let engine = PolicyEngine::from_boundaries(vec![political_boundary()]);

        let task = TaskDescriptor {
            id: "task-nfkc-2".into(),
            task_type: "suggest charity".into(),
            payload: serde_json::json!({
                "query": "aligned with user's \u{FF24}\u{FF4F}nation patterns and political views"
            }),
            submitted_at: Utc::now(),
        };

        match engine.evaluate(&task) {
            PolicyVerdict::Refuse(refusal) => {
                assert_eq!(refusal.boundary_id, "BOUNDARY-001");
            }
            PolicyVerdict::Allow => {
                panic!("NFKC fullwidth evasion should be caught")
            }
        }
    }

    #[test]
    fn punctuation_stripping_catches_embedded_punctuation() {
        // "donation?" should become "donation"
        let engine = PolicyEngine::from_boundaries(vec![political_boundary()]);

        let task = TaskDescriptor {
            id: "task-punct-1".into(),
            task_type: "suggest charity".into(),
            payload: serde_json::json!({
                "query": "based on donation? and political? leanings"
            }),
            submitted_at: Utc::now(),
        };

        match engine.evaluate(&task) {
            PolicyVerdict::Refuse(refusal) => {
                assert_eq!(refusal.boundary_id, "BOUNDARY-001");
            }
            PolicyVerdict::Allow => {
                panic!("Punctuation-embedded keywords should still be caught")
            }
        }
    }
}
