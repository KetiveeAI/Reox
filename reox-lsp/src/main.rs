// REOX Language Server - stdio entry point.

use std::io;
use std::process;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    for arg in &args {
        match arg.as_str() {
            "--version" | "-V" => {
                println!("reox-lsp {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "--help" | "-h" => {
                println!("reox-lsp - REOX language server (LSP over stdio)");
                println!();
                println!("USAGE:");
                println!("    reox-lsp [--stdio]");
                println!();
                println!("The server speaks JSON-RPC 2.0 with Content-Length framing on");
                println!("stdin/stdout. Set REOX_PRELUDE to override the prelude file used");
                println!("for completion.");
                return;
            }
            "--stdio" => {}
            other => {
                eprintln!("reox-lsp: unknown option: {}", other);
                process::exit(2);
            }
        }
    }

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();
    let code = reox_lsp::server::Server::new().run(&mut reader, &mut writer);
    process::exit(code);
}
