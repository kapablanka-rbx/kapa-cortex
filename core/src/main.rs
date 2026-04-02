mod db;
mod index;
mod graph;
mod lsp;
mod server;

use std::path::PathBuf;
use std::sync::Arc;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: kapa-cortex-core <command>");
        eprintln!("Commands: start, stop, status, index [root]");
        std::process::exit(1);
    }

    match args[1].as_str() {
        "start" => start_daemon(),
        "stop" => stop_daemon(),
        "status" => check_status(),
        "index" => {
            let root = args.get(2).map(|s| s.as_str()).unwrap_or(".");
            run_index(root);
        }
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            std::process::exit(1);
        }
    }
}

fn start_daemon() {
    let db_path = PathBuf::from(".cortex-cache/index.db");
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let db = match db::Database::open(&db_path) {
        Ok(db) => db,
        Err(err) => {
            eprintln!("  \x1b[31mFailed to open database: {}\x1b[0m", err);
            std::process::exit(1);
        }
    };

    let db = Arc::new(db);

    eprintln!(
        "  \x1b[32m✓\x1b[0m Database opened: {}",
        db_path.display()
    );

    // Print stats
    db.with_conn(|conn| {
        if let (Ok(files), Ok(symbols), Ok(edges), Ok(calls)) = (
            db::queries::file_count(conn),
            db::queries::symbol_count(conn),
            db::queries::edge_count(conn),
            db::queries::call_count(conn),
        ) {
            eprintln!(
                "  \x1b[32m✓\x1b[0m Index: {} files, {} symbols, {} edges, {} calls",
                files, symbols, edges, calls
            );
        }
    });

    if let Err(err) = server::run(db) {
        eprintln!("  \x1b[31mServer error: {}\x1b[0m", err);
        std::process::exit(1);
    }
}

fn stop_daemon() {
    // Send shutdown via socket
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    let mut stream = match UnixStream::connect(server::SOCKET_PATH) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("  \x1b[33mNo daemon running.\x1b[0m");
            return;
        }
    };

    let payload = serde_json::json!({"action": "shutdown", "params": {}});
    let bytes = serde_json::to_vec(&payload).unwrap();
    let header = (bytes.len() as u64).to_be_bytes();
    stream.write_all(&header).ok();
    stream.write_all(&bytes).ok();

    let mut response = Vec::new();
    stream.read_to_end(&mut response).ok();
    eprintln!("  \x1b[32mDaemon stopped.\x1b[0m");
}

fn run_index(root: &str) {
    let db_path = PathBuf::from(format!("{}/.cortex-cache/index.db", root));
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let db = db::Database::open(&db_path).unwrap_or_else(|e| {
        eprintln!("  \x1b[31mFailed to open database: {}\x1b[0m", e);
        std::process::exit(1);
    });

    // Clear old data
    db.with_conn(|conn| {
        conn.execute_batch(
            "DELETE FROM files; DELETE FROM symbols; DELETE FROM imports; DELETE FROM edges; DELETE FROM calls;"
        ).ok();
    });

    if let Err(e) = index::index_repo(&db, root) {
        eprintln!("  \x1b[31mIndex error: {}\x1b[0m", e);
        std::process::exit(1);
    }

    db.with_conn(|conn| {
        if let (Ok(files), Ok(symbols), Ok(edges)) = (
            db::queries::file_count(conn),
            db::queries::symbol_count(conn),
            db::queries::edge_count(conn),
        ) {
            eprintln!(
                "  \x1b[32m✓\x1b[0m Index complete: {} files, {} symbols, {} edges",
                files, symbols, edges
            );
        }
    });
}

fn check_status() {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    let mut stream = match UnixStream::connect(server::SOCKET_PATH) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("  \x1b[33mNo daemon running.\x1b[0m");
            std::process::exit(1);
        }
    };

    let payload = serde_json::json!({"action": "status", "params": {}});
    let bytes = serde_json::to_vec(&payload).unwrap();
    let header = (bytes.len() as u64).to_be_bytes();
    stream.write_all(&header).ok();
    stream.write_all(&bytes).ok();

    let mut header_buf = [0u8; 8];
    if stream.read_exact(&mut header_buf).is_err() {
        eprintln!("  \x1b[31mFailed to read response\x1b[0m");
        std::process::exit(1);
    }
    let length = u64::from_be_bytes(header_buf) as usize;
    let mut response = vec![0u8; length];
    stream.read_exact(&mut response).ok();

    let parsed: serde_json::Value = serde_json::from_slice(&response).unwrap_or_default();
    if let Some(data) = parsed.get("data") {
        println!("{}", serde_json::to_string_pretty(data).unwrap_or_default());
    }
}
