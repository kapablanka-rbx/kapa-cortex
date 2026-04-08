use std::collections::HashMap;
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::sync::Arc;
use crate::infrastructure::sqlite::Database;
use crate::infrastructure::lsp::LspClient;
use super::handler;

pub const SOCKET_PATH: &str = "/tmp/kapa-cortex.sock";

pub struct DaemonState {
    pub db: Database,
    pub lsp_clients: std::sync::Mutex<HashMap<String, LspClient>>,
}

pub fn run(db: Arc<Database>) -> std::io::Result<()> {
    let socket_path = Path::new(SOCKET_PATH);
    if socket_path.exists() {
        std::fs::remove_file(socket_path)?;
    }

    // Detect all languages in the repo and boot LSP for each
    let languages = crate::infrastructure::lsp::detect_all_languages(".");
    let mut clients: HashMap<String, LspClient> = HashMap::new();

    for lang in &languages {
        eprint!("  \x1b[36mBooting LSP for {}...\x1b[0m", lang);
        match LspClient::start(lang, ".") {
            Some(client) => {
                eprintln!("\r\x1b[2K  \x1b[32m✓\x1b[0m LSP: {} ready", lang);
                clients.insert(lang.to_string(), client);
            }
            None => {
                eprintln!("\r\x1b[2K  \x1b[33m✗\x1b[0m LSP: {} not available", lang);
            }
        }
    }

    if clients.is_empty() {
        eprintln!("  \x1b[33mNo LSP servers available\x1b[0m");
    }

    let state = Arc::new(DaemonState {
        db: Arc::try_unwrap(db).unwrap_or_else(|_arc| {
            Database::open(&Path::new(".cortex-cache/index.db")).unwrap()
        }),
        lsp_clients: std::sync::Mutex::new(clients),
    });

    let listener = UnixListener::bind(socket_path)?;
    eprintln!("  \x1b[32m✓\x1b[0m Daemon listening on {}", SOCKET_PATH);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let state = Arc::clone(&state);
                std::thread::spawn(move || {
                    if let Err(err) = handler::handle_connection(stream, &state) {
                        let msg = err.to_string();
                        if !msg.contains("fill whole buffer") && !msg.contains("broken pipe") {
                            eprintln!("  \x1b[33mHandler error: {}\x1b[0m", msg);
                        }
                    }
                });
            }
            Err(err) => {
                eprintln!("  \x1b[33mAccept error: {}\x1b[0m", err);
            }
        }
    }

    Ok(())
}
