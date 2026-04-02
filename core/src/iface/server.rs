use std::os::unix::net::UnixListener;
use std::path::Path;
use std::sync::Arc;
use crate::infrastructure::sqlite::Database;
use super::handler;

pub const SOCKET_PATH: &str = "/tmp/kapa-cortex.sock";

pub fn run(db: Arc<Database>) -> std::io::Result<()> {
    let socket_path = Path::new(SOCKET_PATH);
    if socket_path.exists() {
        std::fs::remove_file(socket_path)?;
    }

    let listener = UnixListener::bind(socket_path)?;
    eprintln!("  \x1b[32m✓\x1b[0m Daemon listening on {}", SOCKET_PATH);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let db = Arc::clone(&db);
                std::thread::spawn(move || {
                    if let Err(err) = handler::handle_connection(stream, &db) {
                        eprintln!("  \x1b[33mHandler error: {}\x1b[0m", err);
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
