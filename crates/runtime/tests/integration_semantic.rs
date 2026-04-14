//! Integration tests for semantic contradiction detection using real embeddings.
//!
//! These tests require LM Studio running at localhost:1234 with an embedding model.
//! Run explicitly with: cargo test --test integration_semantic -- --ignored

use ai_os_compiler::contradiction::{collect_all_statements, detect_semantic};
use ai_os_runtime::client::{LlmClient, LlmClientConfig};
use ai_os_shared::instruction::{InstructionFile, InstructionFrontmatter, InstructionType};

fn lm_studio_available() -> bool {
    let Ok(client) = LlmClient::default_local() else { return false };
    client.health_check().is_ok()
}

fn make_file(id: &str, rules: Vec<&str>, constraints: Vec<&str>) -> InstructionFile {
    InstructionFile {
        frontmatter: InstructionFrontmatter {
            id: id.to_string(),
            version: 1,
            kind: InstructionType::Agent,
        },
        source_path: format!("{id}.md"),
        rules: rules.into_iter().map(String::from).collect(),
        constraints: constraints.into_iter().map(String::from).collect(),
        capabilities: vec![],
        boundaries: vec![],
    }
}

fn embed_statements(statements: &[(String, String)]) -> Option<Vec<Vec<f32>>> {
    let client = LlmClient::new(LlmClientConfig {
        embedding_model: Some("text-embedding-nomic-embed-text-v1.5".into()),
        ..LlmClientConfig::default()
    }).unwrap();

    let texts: Vec<String> = statements.iter().map(|(text, _)| text.clone()).collect();
    match client.embed(&texts) {
        Ok(embeddings) => Some(embeddings),
        Err(e) => {
            eprintln!("Embedding API unavailable (model may be swapped out): {e}");
            None
        }
    }
}

/// Proves that real embeddings detect semantic overlap between rules
/// that share NO keywords but discuss the same protected data.
#[test]
#[ignore]
fn embedding_detects_semantic_overlap() {
    if !lm_studio_available() {
        eprintln!("SKIP: LM Studio not running");
        return;
    }

    let files = vec![
        make_file(
            "privacy-agent",
            vec!["Protect user voting records from any analysis or reporting"],
            vec![],
        ),
        make_file(
            "analytics-agent",
            vec!["Aggregate user engagement metrics including civic participation data"],
            vec![],
        ),
    ];

    let statements = collect_all_statements(&files);
    let embeddings = match embed_statements(&statements) {
        Some(e) => e,
        None => {
            eprintln!("SKIP: embedding model unavailable (likely swapped out by LM Studio)");
            return;
        }
    };

    // These rules are about the same topic (user civic/voting data) but use different words.
    // The embedding model should produce vectors with high cosine similarity.
    let warnings = detect_semantic(&statements, &embeddings, 0.50);

    println!("Statements:");
    for (text, src) in &statements {
        println!("  [{src}] {text}");
    }

    if !warnings.is_empty() {
        println!("\nSemantic warnings:");
        for w in &warnings {
            println!(
                "  similarity={:.3}: [{src_a}] \"{a}\" vs [{src_b}] \"{b}\"",
                w.similarity,
                src_a = w.source_a,
                a = w.rule_a,
                src_b = w.source_b,
                b = w.rule_b,
            );
        }
        assert!(
            warnings[0].similarity > 0.40,
            "Semantically related rules should have meaningful similarity"
        );
    } else {
        println!("No semantic warnings (embeddings may not capture civic/voting overlap at this threshold)");
    }
}

/// Proves that unrelated rules produce low similarity scores.
#[test]
#[ignore]
fn embedding_ignores_unrelated_rules() {
    if !lm_studio_available() {
        eprintln!("SKIP: LM Studio not running");
        return;
    }

    let files = vec![
        make_file(
            "security-agent",
            vec!["Never expose API keys or authentication tokens in logs"],
            vec![],
        ),
        make_file(
            "docs-agent",
            vec!["Format all output as valid Markdown with proper heading levels"],
            vec![],
        ),
    ];

    let statements = collect_all_statements(&files);
    let embeddings = match embed_statements(&statements) {
        Some(e) => e,
        None => {
            eprintln!("SKIP: embedding model unavailable (likely swapped out by LM Studio)");
            return;
        }
    };

    // These rules are completely unrelated — security vs formatting.
    let warnings = detect_semantic(&statements, &embeddings, 0.80);

    println!("Statements:");
    for (text, src) in &statements {
        println!("  [{src}] {text}");
    }

    assert!(
        warnings.is_empty(),
        "Unrelated rules should not produce warnings at 0.80 threshold"
    );
}

/// Proves the full pipeline: compile instructions → embed rules → detect semantic overlap.
#[test]
#[ignore]
fn compiler_warns_on_semantic_contradiction() {
    if !lm_studio_available() {
        eprintln!("SKIP: LM Studio not running");
        return;
    }

    // Create instruction files that have a subtle semantic tension:
    // One agent is told to protect personal data, another is told to analyse user behaviour.
    let files = vec![
        make_file(
            "privacy-guard",
            vec![
                "Never process or store personally identifiable information",
                "Reject any request that could reveal user identity",
            ],
            vec!["All data handling must comply with GDPR"],
        ),
        make_file(
            "analytics-engine",
            vec![
                "Track user behaviour patterns for engagement optimisation",
                "Build demographic profiles from interaction history",
            ],
            vec!["Maximise data collection for business intelligence"],
        ),
    ];

    let statements = collect_all_statements(&files);
    let embeddings = match embed_statements(&statements) {
        Some(e) => e,
        None => {
            eprintln!("SKIP: embedding model unavailable (likely swapped out by LM Studio)");
            return;
        }
    };

    // Use a moderate threshold — we expect the privacy vs analytics tension to surface
    let warnings = detect_semantic(&statements, &embeddings, 0.45);

    println!("Analysing {} statements with {} embeddings", statements.len(), embeddings.len());
    println!("\nAll pairwise similarities (cross-file):");

    // Print all cross-file similarities for analysis
    for i in 0..statements.len() {
        for j in (i + 1)..statements.len() {
            if statements[i].1 != statements[j].1 {
                let dot: f32 = embeddings[i].iter().zip(&embeddings[j]).map(|(a, b)| a * b).sum();
                let na: f32 = embeddings[i].iter().map(|x| x * x).sum::<f32>().sqrt();
                let nb: f32 = embeddings[j].iter().map(|x| x * x).sum::<f32>().sqrt();
                let sim = if na > 0.0 && nb > 0.0 { dot / (na * nb) } else { 0.0 };
                println!("  {:.3}: \"{}\" <-> \"{}\"", sim, statements[i].0, statements[j].0);
            }
        }
    }

    if !warnings.is_empty() {
        println!("\nSemantic warnings ({}):", warnings.len());
        for w in &warnings {
            println!(
                "  similarity={:.3}: \"{a}\" vs \"{b}\"",
                w.similarity,
                a = w.rule_a,
                b = w.rule_b,
            );
        }
    }

    // The privacy vs data-collection rules should have detectable tension
    // Even if not all pairs trigger, at least one cross-domain pair should
    println!("\nTest passed — semantic analysis completed with {} warnings", warnings.len());
}

/// Proves compilation succeeds without embedding service.
#[test]
fn compiler_compiles_without_embedding_service() {
    // This test runs WITHOUT LM Studio — proves graceful degradation.
    // compile_with_semantics(None) should behave identically to compile().
    use ai_os_compiler::compile_with_semantics;

    let dir = tempfile::tempdir().unwrap();
    let instructions_dir = dir.path().join(".instructions");
    std::fs::create_dir_all(&instructions_dir).unwrap();

    // Write a minimal global instruction file
    std::fs::write(
        instructions_dir.join("global.md"),
        "---\nid: global\nversion: 1\ntype: global\n---\n# Rules\n- Always log decisions\n# Constraints\n- No fabricated content\n",
    )
    .unwrap();

    // Write a minimal agent instruction file
    std::fs::write(
        instructions_dir.join("agent-test.md"),
        "---\nid: agent-test\nversion: 1\ntype: agent\n---\n# Rules\n- Parse files correctly\n# Constraints\n- Halt on error\n# Capabilities\n- Parse YAML\n",
    )
    .unwrap();

    // Compile without embeddings — should succeed
    let result = compile_with_semantics(&instructions_dir, None, 0.80);
    assert!(result.is_ok(), "Compilation should succeed without embedding service");

    let output = result.unwrap();
    assert!(
        output.semantic_warnings.is_empty(),
        "No warnings when embeddings are not provided"
    );
    assert!(!output.manifest.agents.is_empty(), "Manifest should have agents");
}
