pub mod contradiction;
pub mod manifest;
pub mod parser;
pub mod validator;

use ai_os_shared::contract::ContractManifest;
use ai_os_shared::error::{AiOsError, Result};
use std::path::Path;

pub use contradiction::SemanticWarning;

/// The output of a compilation that may include semantic warnings.
#[derive(Debug)]
pub struct CompileOutput {
    /// The compiled contract manifest.
    pub manifest: ContractManifest,
    /// Advisory warnings about semantically similar rules (not blocking).
    pub semantic_warnings: Vec<SemanticWarning>,
}

/// Shared pipeline: parse → validate → detect contradictions.
///
/// Halts on the first error (fail-closed).
fn compile_internal(
    instructions_dir: &Path,
) -> Result<Vec<ai_os_shared::instruction::InstructionFile>> {
    let files = parser::parse_directory(instructions_dir)?;

    if files.is_empty() {
        return Err(AiOsError::Validation {
            file: instructions_dir.display().to_string(),
            message: "No instruction files found".into(),
        });
    }

    for file in &files {
        validator::validate(file)?;
    }

    contradiction::detect(&files)?;

    Ok(files)
}

/// Compile instruction files from `instructions_dir` into a contract manifest.
///
/// Pipeline: parse → validate → detect contradictions → generate manifest.
/// Halts on the first error (fail-closed).
pub fn compile(instructions_dir: &Path) -> Result<ContractManifest> {
    let files = compile_internal(instructions_dir)?;
    Ok(manifest::generate(&files))
}

/// Convenience: compile and write the manifest to a JSON file.
pub fn compile_to_file(instructions_dir: &Path, output_path: &Path) -> Result<()> {
    let manifest = compile(instructions_dir)?;
    let json = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(output_path, json).map_err(AiOsError::Io)?;
    Ok(())
}

/// Compile with optional semantic analysis using pre-computed embeddings.
///
/// If `embeddings` is `Some`, runs embedding-based contradiction detection
/// after the standard keyword detection. Semantic issues are returned as
/// warnings in `CompileOutput::semantic_warnings` — they do NOT halt compilation.
///
/// If `embeddings` is `None`, behaves identically to `compile()`.
pub fn compile_with_semantics(
    instructions_dir: &Path,
    embeddings: Option<&[Vec<f32>]>,
    similarity_threshold: f32,
) -> Result<CompileOutput> {
    let files = compile_internal(instructions_dir)?;

    // Semantic analysis (advisory)
    let semantic_warnings = if let Some(emb) = embeddings {
        let statements = contradiction::collect_all_statements(&files);
        if statements.len() == emb.len() {
            contradiction::detect_semantic(&statements, emb, similarity_threshold)
        } else {
            // Embedding count mismatch — skip semantic analysis silently
            Vec::new()
        }
    } else {
        Vec::new()
    };

    // Step 5: Generate manifest
    let manifest = manifest::generate(&files);

    Ok(CompileOutput {
        manifest,
        semantic_warnings,
    })
}
