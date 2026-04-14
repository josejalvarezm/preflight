use ai_os_runtime::client::LlmClient;

fn main() {
    println!("AI-OS Runtime — LLM Agent Executor");
    println!("===================================");

    // Health check
    let client = match LlmClient::default_local() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ERROR: Failed to initialise HTTP client: {e}");
            std::process::exit(1);
        }
    };
    match client.health_check() {
        Ok(models) => {
            println!("Connected to LM Studio. Available models:");
            for m in &models {
                println!("  - {m}");
            }
        }
        Err(e) => {
            eprintln!("ERROR: Cannot connect to LLM service: {e}");
            eprintln!("Ensure LM Studio is running at {}", client.config().base_url);
            std::process::exit(1);
        }
    }
}
