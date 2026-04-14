use ai_os_compiler::compile;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Guard against common mistake: `cargo run -p ai-os-compiler -- compile dir/ out.json`
    if args.len() > 1 && args[1] == "compile" {
        eprintln!(
            "Error: 'compile' is not a subcommand.\n\
             Usage: cargo run -p ai-os-compiler -- <instructions_dir> <output_path>\n\
             Example: cargo run -p ai-os-compiler -- .instructions/ .instructions/contracts/contract.json"
        );
        std::process::exit(1);
    }

    let instructions_dir = args.get(1)
        .map(|s| s.as_str())
        .unwrap_or(".instructions");
    let output_path = args.get(2)
        .map(|s| s.as_str())
        .unwrap_or(".instructions/contracts/contract.json");

    let instructions = Path::new(instructions_dir);
    let output = Path::new(output_path);

    match compile(instructions) {
        Ok(manifest) => {
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent).expect("Failed to create output directory");
            }
            let json = serde_json::to_string_pretty(&manifest).expect("Failed to serialise manifest");
            std::fs::write(output, &json).expect("Failed to write manifest");
            println!("Compiled {} agent(s) + global rules → {}", manifest.agents.len(), output.display());
            println!("Version: {}", manifest.version);
            println!("Compiled at: {}", manifest.compiled_at);
        }
        Err(e) => {
            eprintln!("Compilation failed: {e}");
            std::process::exit(1);
        }
    }
}
