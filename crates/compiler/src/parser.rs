use ai_os_shared::boundary::BoundaryCategory;
use ai_os_shared::error::{AiOsError, Result};
use ai_os_shared::instruction::{BoundaryDefinition, InstructionFile, InstructionFrontmatter};
use std::path::Path;
use walkdir::WalkDir;

/// Parse all `.md` files in the instructions directory into `InstructionFile`s.
pub fn parse_directory(dir: &Path) -> Result<Vec<InstructionFile>> {
    let mut files = Vec::new();

    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension().is_some_and(|ext| ext == "md")
                && e.file_type().is_file()
        })
    {
        // Skip files inside the contracts/ output directory
        if entry
            .path()
            .components()
            .any(|c| c.as_os_str() == "contracts")
        {
            continue;
        }

        let parsed = parse_file(entry.path())?;
        files.push(parsed);
    }

    Ok(files)
}

/// Parse a single instruction file: extract YAML frontmatter and Markdown sections.
pub fn parse_file(path: &Path) -> Result<InstructionFile> {
    let content = std::fs::read_to_string(path).map_err(AiOsError::Io)?;
    let source_path = path.display().to_string();

    let (frontmatter, body) = split_frontmatter(&content).ok_or_else(|| AiOsError::Validation {
        file: source_path.clone(),
        message: "Missing or malformed YAML frontmatter (expected --- delimiters)".into(),
    })?;

    let fm: InstructionFrontmatter =
        serde_yaml::from_str(&frontmatter).map_err(|e| AiOsError::Yaml(format!("{source_path}: {e}")))?;

    let rules = extract_section(&body, "Rules");
    let constraints = extract_section(&body, "Constraints");
    let capabilities = extract_section(&body, "Capabilities");
    let boundaries = extract_boundaries(&body);

    Ok(InstructionFile {
        frontmatter: fm,
        source_path,
        rules,
        constraints,
        capabilities,
        boundaries,
    })
}

/// Split `---` delimited YAML frontmatter from the Markdown body.
fn split_frontmatter(content: &str) -> Option<(String, String)> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }

    // Find the closing ---
    let after_first = &trimmed[3..];
    let end = after_first.find("\n---")?;
    let yaml = after_first[..end].trim().to_string();
    let body = after_first[end + 4..].to_string();

    Some((yaml, body))
}

/// Extract bullet-point items under a `# <heading>` section.
fn extract_section(body: &str, heading: &str) -> Vec<String> {
    let target = format!("# {heading}");
    let mut in_section = false;
    let mut items = Vec::new();

    for line in body.lines() {
        let trimmed = line.trim();

        if trimmed == target {
            in_section = true;
            continue;
        }

        // A new heading ends the current section
        if trimmed.starts_with("# ") && in_section {
            break;
        }

        if in_section && trimmed.starts_with("- ") {
            items.push(trimmed[2..].trim().to_string());
        }
    }

    items
}

/// Extract boundary definitions from a `# Boundaries` section.
///
/// Expected format — each boundary is a bullet with sub-items:
/// ```text
/// # Boundaries
/// - id: BOUNDARY-001
///   category: privacy
///   triggers: charity, donation, political, party, voting
///   subjects: political, party, voting, donation
///   rule: Never share the user's political affiliation.
/// ```
fn extract_boundaries(body: &str) -> Vec<BoundaryDefinition> {
    let target = "# Boundaries";
    let mut in_section = false;
    let mut boundaries = Vec::new();

    // Collect raw text blocks for each boundary (one per `- id:` bullet)
    let mut current_block: Vec<String> = Vec::new();

    for line in body.lines() {
        let trimmed = line.trim();

        if trimmed == target {
            in_section = true;
            continue;
        }

        // A new heading ends the boundaries section
        if trimmed.starts_with("# ") && in_section {
            break;
        }

        if !in_section {
            continue;
        }

        // New boundary starts with `- id:`
        if trimmed.starts_with("- id:") {
            if !current_block.is_empty() {
                if let Some(b) = parse_boundary_block(&current_block) {
                    boundaries.push(b);
                }
            }
            current_block = vec![trimmed.to_string()];
        } else if !current_block.is_empty() && !trimmed.is_empty() {
            // Continuation lines (indented sub-fields)
            current_block.push(trimmed.to_string());
        }
    }

    // Don't forget the last block
    if !current_block.is_empty() {
        if let Some(b) = parse_boundary_block(&current_block) {
            boundaries.push(b);
        }
    }

    boundaries
}

/// Parse a single boundary block into a `BoundaryDefinition`.
fn parse_boundary_block(lines: &[String]) -> Option<BoundaryDefinition> {
    let mut id = None;
    let mut category = None;
    let mut triggers = Vec::new();
    let mut subjects = Vec::new();
    let mut rule = None;

    for line in lines {
        // Strip leading `- ` from the first line
        let cleaned = if let Some(stripped) = line.strip_prefix("- ") {
            stripped
        } else {
            line.as_str()
        };

        if let Some(val) = cleaned.strip_prefix("id:") {
            id = Some(val.trim().to_string());
        } else if let Some(val) = cleaned.strip_prefix("category:") {
            category = Some(parse_category(val.trim()));
        } else if let Some(val) = cleaned.strip_prefix("triggers:") {
            triggers = val
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
        } else if let Some(val) = cleaned.strip_prefix("subjects:") {
            subjects = val
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
        } else if let Some(val) = cleaned.strip_prefix("rule:") {
            rule = Some(val.trim().to_string());
        }
    }

    Some(BoundaryDefinition {
        id: id?,
        category: category?,
        trigger_patterns: triggers,
        protected_subjects: subjects,
        source_rule: rule.unwrap_or_default(),
    })
}

fn parse_category(s: &str) -> BoundaryCategory {
    match s.to_lowercase().as_str() {
        "privacy" => BoundaryCategory::Privacy,
        "security" => BoundaryCategory::Security,
        "legal" => BoundaryCategory::Legal,
        other => BoundaryCategory::Custom(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_os_shared::instruction::InstructionType;

    const SAMPLE: &str = r#"---
id: test-agent
version: 1
type: agent
---

# Rules
- Do something important.
- Do it correctly.

# Constraints
- Never fail silently.

# Capabilities
- Parse files
"#;

    #[test]
    fn parse_frontmatter_and_sections() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        std::fs::write(&file_path, SAMPLE).unwrap();

        let result = parse_file(&file_path).unwrap();

        assert_eq!(result.frontmatter.id, "test-agent");
        assert_eq!(result.frontmatter.version, 1);
        assert_eq!(result.frontmatter.kind, InstructionType::Agent);
        assert_eq!(result.rules.len(), 2);
        assert_eq!(result.constraints.len(), 1);
        assert_eq!(result.capabilities.len(), 1);
        assert_eq!(result.rules[0], "Do something important.");
        assert!(result.boundaries.is_empty());
    }

    #[test]
    fn parse_boundary_section() {
        let input = r#"---
id: global-policy
version: 1
type: global
---

# Rules
- Protect user privacy at all times.

# Constraints
- Never share PII without consent.

# Boundaries
- id: BOUNDARY-001
  category: privacy
  triggers: charity, donation, political, party, voting
  subjects: political, party, voting, donation
  rule: Never share the user's political affiliation.
- id: BOUNDARY-002
  category: security
  triggers: password, credential, token, secret
  subjects: password, credential, secret
  rule: Never expose authentication credentials.
"#;
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("global.md");
        std::fs::write(&file_path, input).unwrap();

        let result = parse_file(&file_path).unwrap();

        assert_eq!(result.boundaries.len(), 2);
        assert_eq!(result.boundaries[0].id, "BOUNDARY-001");
        assert_eq!(result.boundaries[0].category, BoundaryCategory::Privacy);
        assert_eq!(result.boundaries[0].trigger_patterns.len(), 5);
        assert!(result.boundaries[0].trigger_patterns.contains(&"charity".to_string()));
        assert_eq!(result.boundaries[0].protected_subjects.len(), 4);
        assert_eq!(
            result.boundaries[0].source_rule,
            "Never share the user's political affiliation."
        );

        assert_eq!(result.boundaries[1].id, "BOUNDARY-002");
        assert_eq!(result.boundaries[1].category, BoundaryCategory::Security);
        assert_eq!(result.boundaries[1].trigger_patterns.len(), 4);
    }

    #[test]
    fn missing_frontmatter_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("bad.md");
        std::fs::write(&file_path, "# Just a heading\n- no frontmatter").unwrap();

        let result = parse_file(&file_path);
        assert!(result.is_err());
    }
}
