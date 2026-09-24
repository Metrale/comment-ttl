// 2026-09-24: Binary entry point: parse the command line, run, map errors to
// exit code 2 with a GitHub error annotation when inside Actions.

use clap::Parser;

use comment_ttl::cli::{Cli, invocation};
use comment_ttl::run::{EXIT_CONFIG, run};

fn main() {
    let cli = Cli::parse();
    let env = |k: &str| std::env::var(k).ok();
    let in_actions = std::env::var_os("GITHUB_ACTIONS").is_some();
    match run(invocation(cli, &env)) {
        Ok(outcome) => std::process::exit(outcome.exit_code),
        Err(message) => {
            eprintln!("comment-ttl: configuration error: {message}");
            if in_actions {
                println!(
                    "::error title=comment-ttl configuration error::{}",
                    message.replace('%', "%25").replace('\n', "%0A")
                );
            }
            std::process::exit(EXIT_CONFIG);
        }
    }
}
