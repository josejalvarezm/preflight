use ai_os_kernel::Kernel;
use ai_os_shared::task::TaskDescriptor;
use std::io::{self, BufRead, Write};
use std::path::Path;

fn main() {
    let manifest_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| ".instructions/contracts/contract.json".to_string());
    let log_path = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "decisions.jsonl".to_string());

    let manifest = Path::new(&manifest_path);
    let log = Path::new(&log_path);

    if !manifest.exists() {
        eprintln!(
            "Contract manifest not found: {}\n\
             Run `cargo run -p ai-os-compiler` first to compile instructions.",
            manifest.display()
        );
        std::process::exit(1);
    }

    let mut kernel = match Kernel::boot(manifest, log) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("Kernel boot failed: {e}");
            std::process::exit(1);
        }
    };

    let agent_count = kernel.roles().agent_ids().len();
    let boundary_count = kernel.policy_engine().active_count();
    eprintln!("AI-OS Kernel booted: {agent_count} agent(s) loaded, {boundary_count} boundary(ies) active.");
    eprintln!("Decision log: {}", log.display());
    eprintln!("Submit JSON task descriptors on stdin (one per line), or Ctrl+D to exit.");
    eprintln!();

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Read error: {e}");
                break;
            }
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let task: TaskDescriptor = match serde_json::from_str(trimmed) {
            Ok(t) => t,
            Err(e) => {
                let err = serde_json::json!({
                    "error": "invalid_task",
                    "detail": e.to_string()
                });
                writeln!(out, "{}", err).ok();
                continue;
            }
        };

        match kernel.route(&task) {
            Ok(decision) => {
                let result = serde_json::json!({
                    "task_id": task.id,
                    "routed_to": decision.agent_id,
                    "rationale": decision.rationale
                });
                writeln!(out, "{}", result).ok();
            }
            Err(ai_os_kernel::RoutingError::PolicyRefusal(refusal)) => {
                let result = serde_json::json!({
                    "task_id": task.id,
                    "error": "policy_refusal",
                    "boundary_id": refusal.boundary_id,
                    "category": format!("{:?}", refusal.category),
                    "detail": refusal.reason,
                    "directive": format!("{:?}", refusal.agent_directive)
                });
                writeln!(out, "{}", result).ok();
            }
            Err(e) => {
                let result = serde_json::json!({
                    "task_id": task.id,
                    "error": "routing_failed",
                    "detail": e.to_string()
                });
                writeln!(out, "{}", result).ok();
            }
        }
    }

    eprintln!("Kernel shut down.");
}
