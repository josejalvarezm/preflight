use ai_os_shared::error::{AiOsError, Result};
use ai_os_shared::instruction::InstructionFile;

/// A semantic similarity warning (advisory, not blocking).
#[derive(Debug, Clone)]
pub struct SemanticWarning {
    /// First rule text.
    pub rule_a: String,
    /// Source file of the first rule.
    pub source_a: String,
    /// Second rule text.
    pub rule_b: String,
    /// Source file of the second rule.
    pub source_b: String,
    /// Cosine similarity score (0.0 to 1.0).
    pub similarity: f32,
}

/// Detect contradictions across a set of instruction files.
///
/// Current checks:
/// 1. Duplicate IDs — two files with the same `id` is a conflict.
/// 2. Negation patterns — a rule "always X" in one file vs "never X" in another.
///
/// Returns `Ok(())` if no contradictions, or the first contradiction found.
pub fn detect(files: &[InstructionFile]) -> Result<()> {
    check_duplicate_ids(files)?;
    check_negation_patterns(files)?;
    Ok(())
}

fn check_duplicate_ids(files: &[InstructionFile]) -> Result<()> {
    for (i, a) in files.iter().enumerate() {
        for b in files.iter().skip(i + 1) {
            if a.frontmatter.id == b.frontmatter.id {
                return Err(AiOsError::Contradiction {
                    file_a: a.source_path.clone(),
                    file_b: b.source_path.clone(),
                    description: format!("Duplicate instruction id '{}'", a.frontmatter.id),
                });
            }
        }
    }
    Ok(())
}

/// Simple negation detection: looks for "always" vs "never" on the same
/// normalised phrase across rules and constraints of different files.
fn check_negation_patterns(files: &[InstructionFile]) -> Result<()> {
    // Collect all (normalised_phrase, polarity, source_path) tuples
    let mut assertions: Vec<(String, Polarity, &str)> = Vec::new();

    for file in files {
        let all_statements = file.rules.iter().chain(file.constraints.iter());
        for stmt in all_statements {
            if let Some((phrase, polarity)) = extract_polarity(stmt) {
                assertions.push((phrase, polarity, &file.source_path));
            }
        }
    }

    // Check for contradictions: same phrase, opposite polarity, different files
    for (i, (phrase_a, pol_a, src_a)) in assertions.iter().enumerate() {
        for (phrase_b, pol_b, src_b) in assertions.iter().skip(i + 1) {
            if phrase_a == phrase_b && pol_a != pol_b && src_a != src_b {
                return Err(AiOsError::Contradiction {
                    file_a: src_a.to_string(),
                    file_b: src_b.to_string(),
                    description: format!(
                        "Conflicting polarity on '{}': one says {:?}, other says {:?}",
                        phrase_a, pol_a, pol_b
                    ),
                });
            }
        }
    }

    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum Polarity {
    Always,
    Never,
}

/// Extract a normalised phrase and polarity from statements like
/// "Always log decisions" or "Never modify source files".
fn extract_polarity(statement: &str) -> Option<(String, Polarity)> {
    let lower = statement.to_lowercase();
    let trimmed = lower.trim();

    if let Some(rest) = trimmed.strip_prefix("always ") {
        Some((normalise(rest), Polarity::Always))
    } else {
        trimmed
            .strip_prefix("never ")
            .map(|rest| (normalise(rest), Polarity::Never))
    }
}

fn normalise(s: &str) -> String {
    s.trim_end_matches('.').trim().to_string()
}

/// Collect all rule and constraint strings with their source file paths.
/// Used for both keyword and semantic analysis.
pub fn collect_all_statements(files: &[InstructionFile]) -> Vec<(String, String)> {
    let mut statements = Vec::new();
    for file in files {
        for rule in &file.rules {
            statements.push((rule.clone(), file.source_path.clone()));
        }
        for constraint in &file.constraints {
            statements.push((constraint.clone(), file.source_path.clone()));
        }
    }
    statements
}

/// Detect semantically similar rules using pre-computed embeddings.
///
/// This is an **advisory** layer — it returns warnings, not errors.
/// The caller decides how to present them (print, log, or ignore).
///
/// # Arguments
/// * `statements` — (rule_text, source_file) pairs from `collect_all_statements`
/// * `embeddings` — embedding vectors in the same order as `statements`
/// * `threshold` — cosine similarity threshold (e.g. 0.82). Pairs above this score are flagged.
///
/// # Behaviour
/// - Only flags pairs from **different** source files
/// - Skips pairs that would already be caught by keyword negation detection
/// - Returns warnings sorted by similarity score (highest first)
pub fn detect_semantic(
    statements: &[(String, String)],
    embeddings: &[Vec<f32>],
    threshold: f32,
) -> Vec<SemanticWarning> {
    assert_eq!(
        statements.len(),
        embeddings.len(),
        "Statement count must match embedding count"
    );

    let mut warnings = Vec::new();

    for i in 0..statements.len() {
        for j in (i + 1)..statements.len() {
            let (ref rule_a, ref src_a) = statements[i];
            let (ref rule_b, ref src_b) = statements[j];

            // Only flag cross-file pairs
            if src_a == src_b {
                continue;
            }

            // Skip if keyword detection would already catch this
            let a_pol = extract_polarity(rule_a);
            let b_pol = extract_polarity(rule_b);
            if let (Some((phrase_a, pol_a)), Some((phrase_b, pol_b))) = (&a_pol, &b_pol) {
                if phrase_a == phrase_b && pol_a != pol_b {
                    continue; // Already caught by keyword detector
                }
            }

            let sim = cosine_similarity(&embeddings[i], &embeddings[j]);
            if sim >= threshold {
                warnings.push(SemanticWarning {
                    rule_a: rule_a.clone(),
                    source_a: src_a.clone(),
                    rule_b: rule_b.clone(),
                    source_b: src_b.clone(),
                    similarity: sim,
                });
            }
        }
    }

    // Sort by similarity descending (most concerning first)
    warnings.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));

    warnings
}

/// Cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_os_shared::instruction::{InstructionFile, InstructionFrontmatter, InstructionType};

    fn make_file(id: &str, rules: Vec<&str>) -> InstructionFile {
        InstructionFile {
            frontmatter: InstructionFrontmatter {
                id: id.to_string(),
                version: 1,
                kind: InstructionType::Agent,
            },
            source_path: format!("{id}.md"),
            rules: rules.into_iter().map(String::from).collect(),
            constraints: vec!["placeholder".to_string()],
            capabilities: vec!["placeholder".to_string()],
            boundaries: vec![],
        }
    }

    #[test]
    fn duplicate_ids_detected() {
        let files = vec![make_file("agent-a", vec!["rule"]), make_file("agent-a", vec!["rule"])];
        let result = detect(&files);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Duplicate"));
    }

    #[test]
    fn negation_contradiction_detected() {
        let a = make_file("agent-a", vec!["Always log decisions."]);
        let b = make_file("agent-b", vec!["Never log decisions."]);
        let result = detect(&[a, b]);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Conflicting polarity"));
    }

    #[test]
    fn no_contradiction_when_compatible() {
        let a = make_file("agent-a", vec!["Always log decisions."]);
        let b = make_file("agent-b", vec!["Always validate inputs."]);
        assert!(detect(&[a, b]).is_ok());
    }

    #[test]
    fn collect_statements_gathers_rules_and_constraints() {
        let a = make_file("agent-a", vec!["Rule one"]);
        let stmts = collect_all_statements(&[a]);
        // 1 rule + 1 constraint ("placeholder")
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0].0, "Rule one");
    }

    #[test]
    fn semantic_detection_flags_similar_embeddings() {
        let statements = vec![
            ("Protect user voting records".to_string(), "file-a.md".to_string()),
            ("Aggregate civic participation data".to_string(), "file-b.md".to_string()),
        ];
        // Simulate high-similarity embeddings (cosine ~0.95)
        let embeddings = vec![
            vec![0.8, 0.5, 0.3, 0.1],
            vec![0.78, 0.52, 0.28, 0.12],
        ];
        let warnings = detect_semantic(&statements, &embeddings, 0.90);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].similarity > 0.90);
    }

    #[test]
    fn semantic_detection_ignores_low_similarity() {
        let statements = vec![
            ("Always log decisions".to_string(), "file-a.md".to_string()),
            ("Parse YAML files correctly".to_string(), "file-b.md".to_string()),
        ];
        // Simulate orthogonal embeddings
        let embeddings = vec![
            vec![1.0, 0.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0, 0.0],
        ];
        let warnings = detect_semantic(&statements, &embeddings, 0.80);
        assert!(warnings.is_empty());
    }

    #[test]
    fn semantic_detection_skips_same_file_pairs() {
        let statements = vec![
            ("Rule one".to_string(), "same-file.md".to_string()),
            ("Rule two".to_string(), "same-file.md".to_string()),
        ];
        // High similarity but same file
        let embeddings = vec![
            vec![0.9, 0.4, 0.1],
            vec![0.88, 0.42, 0.12],
        ];
        let warnings = detect_semantic(&statements, &embeddings, 0.80);
        assert!(warnings.is_empty(), "Should not flag same-file pairs");
    }

    #[test]
    fn semantic_detection_skips_keyword_caught_contradictions() {
        let statements = vec![
            ("Always log decisions".to_string(), "file-a.md".to_string()),
            ("Never log decisions".to_string(), "file-b.md".to_string()),
        ];
        // High similarity (they're about the same thing)
        let embeddings = vec![
            vec![0.9, 0.4, 0.1],
            vec![0.88, 0.42, 0.12],
        ];
        let warnings = detect_semantic(&statements, &embeddings, 0.80);
        assert!(warnings.is_empty(), "Keyword-detectable contradictions should be skipped");
    }
}
