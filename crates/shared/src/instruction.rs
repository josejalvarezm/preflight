use serde::{Deserialize, Serialize};

use crate::boundary::BoundaryCategory;

/// YAML frontmatter parsed from an instruction file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstructionFrontmatter {
    pub id: String,
    pub version: u32,
    #[serde(rename = "type")]
    pub kind: InstructionType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InstructionType {
    Global,
    Agent,
}

/// A boundary definition as written in an instruction file's `# Boundaries` section.
///
/// These are the human-authored source form. The compiler converts them into
/// `PolicyBoundary` structs in the contract manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BoundaryDefinition {
    /// Human-readable ID, e.g. "BOUNDARY-001".
    pub id: String,
    /// Which category of data/behaviour this protects.
    pub category: BoundaryCategory,
    /// Lowercased keywords that signal a task might touch this boundary.
    pub trigger_patterns: Vec<String>,
    /// Lowercased keywords describing the protected data subject.
    pub protected_subjects: Vec<String>,
    /// The original human-readable rule text.
    pub source_rule: String,
}

/// A fully parsed instruction file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstructionFile {
    pub frontmatter: InstructionFrontmatter,
    pub source_path: String,
    pub rules: Vec<String>,
    pub constraints: Vec<String>,
    /// Only present for agent-type instructions.
    pub capabilities: Vec<String>,
    /// Policy boundaries defined in this file (typically in global instructions).
    #[serde(default)]
    pub boundaries: Vec<BoundaryDefinition>,
}
