use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Categories of enforceable boundaries.
/// Each maps to a class of data or behaviour the system must protect.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum BoundaryCategory {
    /// Personal data that can reveal identity, beliefs, or affiliations.
    Privacy,
    /// Actions that could compromise system integrity.
    Security,
    /// Regulatory or compliance requirements.
    Legal,
    /// User-defined custom boundary.
    Custom(String),
}

/// A compiled policy boundary — the boolean form of a natural-language rule.
///
/// Each boundary carries a set of `trigger_patterns` (lowercased keywords).
/// If a task's type + payload text overlaps with these patterns AND the
/// task touches the protected `category`, the boundary fires.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyBoundary {
    /// Unique identifier, e.g. "BOUNDARY-001".
    pub id: String,
    /// Which category of data/behaviour this protects.
    pub category: BoundaryCategory,
    /// Lowercased keyword patterns that signal a task *might* touch this boundary.
    /// Examples: ["political", "affiliation", "party", "donation", "voting"]
    pub trigger_patterns: Vec<String>,
    /// Lowercased keywords that describe the protected data subject.
    /// Examples: ["political", "religion", "health"]
    pub protected_subjects: Vec<String>,
    /// The original human-readable rule from the diary/instruction file.
    pub source_rule: String,
    /// When this boundary was compiled.
    pub compiled_at: DateTime<Utc>,
    /// Whether this boundary is active. Superseded boundaries are set to false
    /// but never deleted (append-only).
    pub active: bool,
}

/// A structured refusal — the concrete "No" returned when a boundary fires.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefusalRecord {
    /// The task that was refused.
    pub task_id: String,
    /// Which boundary triggered the refusal.
    pub boundary_id: String,
    /// The boundary category.
    pub category: BoundaryCategory,
    /// Human-readable explanation of why the task was refused.
    pub reason: String,
    /// The specific patterns that matched.
    pub matched_patterns: Vec<String>,
    /// When the refusal occurred.
    pub refused_at: DateTime<Utc>,
    /// What the agent should do next.
    pub agent_directive: AgentDirective,
}

/// What happens to the agent after a refusal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentDirective {
    /// The agent must reformulate the request without touching the protected subject.
    Reformulate {
        /// Subjects the agent must exclude from any reformulated request.
        excluded_subjects: Vec<String>,
    },
    /// The agent is terminated for this task — no retry permitted.
    Terminate,
    /// The request is escalated to the user for explicit consent.
    EscalateToUser,
}

/// A record of a rule being superseded (Rigidity Wall solution).
/// The old rule is never deleted — a new version is appended, and the old
/// one is marked inactive. Both the old and new versions remain in the diary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSupersession {
    /// ID of the boundary being superseded.
    pub old_boundary_id: String,
    /// ID of the new boundary replacing it.
    pub new_boundary_id: String,
    /// Who authorised the change.
    pub authorised_by: String,
    /// Why the rule was changed.
    pub reason: String,
    /// When the supersession occurred.
    pub superseded_at: DateTime<Utc>,
}
