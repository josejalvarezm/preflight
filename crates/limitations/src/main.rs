use ai_os_limitations::LimitationTracker;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    let registry_path = std::env::var("AIOS_LIMITATIONS_PATH")
        .unwrap_or_else(|_| "limitations.json".to_string());
    let path = Path::new(&registry_path);

    let command = args[1].as_str();

    match command {
        "declare" => {
            if args.len() < 4 {
                eprintln!("Usage: ai-os-limitations declare <component> <description>");
                std::process::exit(1);
            }
            let mut tracker = LimitationTracker::open(path).expect("Failed to open registry");
            let id = tracker.declare(&args[2], &args[3]);
            tracker.save().expect("Failed to save registry");
            println!("Declared: {id}");
        }
        "link" => {
            if args.len() < 4 {
                eprintln!("Usage: ai-os-limitations link <LIM-ID> <commit-sha>");
                std::process::exit(1);
            }
            let mut tracker = LimitationTracker::open(path).expect("Failed to open registry");
            tracker
                .link_commit(&args[2], &args[3])
                .expect("Failed to link commit");
            tracker.save().expect("Failed to save registry");
            println!("Linked {} → {}", args[2], args[3]);
        }
        "resolve" => {
            if args.len() < 5 {
                eprintln!("Usage: ai-os-limitations resolve <LIM-ID> <commit-sha> <note>");
                std::process::exit(1);
            }
            let mut tracker = LimitationTracker::open(path).expect("Failed to open registry");
            tracker
                .resolve(&args[2], &args[3], &args[4])
                .expect("Failed to resolve limitation");
            tracker.save().expect("Failed to save registry");
            println!("Resolved: {}", args[2]);
        }
        "list" => {
            let tracker = LimitationTracker::open(path).expect("Failed to open registry");
            let entries = tracker.list();
            if entries.is_empty() {
                println!("No limitations registered.");
                return;
            }
            for lim in entries {
                println!(
                    "{} [{}] ({}) — {}",
                    lim.id,
                    format!("{:?}", lim.status).to_lowercase(),
                    lim.component,
                    lim.description
                );
                if !lim.commits.is_empty() {
                    println!("  commits: {}", lim.commits.join(", "));
                }
                if let Some(ref res) = lim.resolution {
                    println!("  resolved: {} — {}", res.commit_sha, res.note);
                }
            }
        }
        _ => {
            eprintln!("Unknown command: {command}");
            print_usage();
            std::process::exit(1);
        }
    }
}

fn print_usage() {
    eprintln!(
        "\
AI-OS Limitation Tracker (C5)

Usage:
  ai-os-limitations declare <component> <description>
  ai-os-limitations link <LIM-ID> <commit-sha>
  ai-os-limitations resolve <LIM-ID> <commit-sha> <note>
  ai-os-limitations list

Environment:
  AIOS_LIMITATIONS_PATH  Path to registry file (default: limitations.json)"
    );
}
