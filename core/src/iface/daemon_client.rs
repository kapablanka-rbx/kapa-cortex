use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use crate::iface::server::SOCKET_PATH;

/// Ensure daemon is running, auto-start if not.
pub fn ensure_running() {
    if UnixStream::connect(SOCKET_PATH).is_ok() {
        return;
    }

    let db_path = PathBuf::from(".cortex-cache/index.db");
    let needs_index = !db_path.exists() || {
        let db = crate::infrastructure::sqlite::Database::open(&db_path).ok();
        db.map(|d| d.with_conn(|c| {
            crate::infrastructure::sqlite::file_count(c).unwrap_or(0) == 0
        })).unwrap_or(true)
    };

    if needs_index {
        crate::application::indexer::index_repo(
            &crate::infrastructure::sqlite::Database::open(&db_path).unwrap(),
            ".",
        ).ok();
    }

    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("kapa-cortex"));
    std::process::Command::new(&exe)
        .args(["daemon", "start"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .ok();

    for _ in 0..100 {
        if UnixStream::connect(SOCKET_PATH).is_ok() {
            std::thread::sleep(std::time::Duration::from_millis(500));
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    eprintln!("Warning: daemon started but socket not ready");
}

/// Send a query to the daemon and return the data field.
pub fn query(action: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    ensure_running();

    let mut stream = connect_with_retry()?;

    let payload = serde_json::json!({"action": action, "params": params});
    let bytes = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
    let header = (bytes.len() as u64).to_be_bytes();
    stream.write_all(&header).map_err(|e| e.to_string())?;
    stream.write_all(&bytes).map_err(|e| e.to_string())?;

    let mut header_buf = [0u8; 8];
    stream.read_exact(&mut header_buf).map_err(|e| e.to_string())?;
    let length = u64::from_be_bytes(header_buf) as usize;
    let mut response = vec![0u8; length];
    read_full(&mut stream, &mut response)?;

    let parsed: serde_json::Value = serde_json::from_slice(&response)
        .map_err(|e| e.to_string())?;

    if parsed.get("status").and_then(|s| s.as_str()) == Some("error") {
        let error = parsed.get("error").and_then(|e| e.as_str()).unwrap_or("unknown");
        return Err(error.to_string());
    }

    Ok(parsed.get("data").cloned().unwrap_or(serde_json::Value::Null))
}

fn connect_with_retry() -> Result<UnixStream, String> {
    for _ in 0..10 {
        if let Ok(s) = UnixStream::connect(SOCKET_PATH) {
            return Ok(s);
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    Err("Daemon not responding".to_string())
}

fn read_full(stream: &mut UnixStream, buf: &mut [u8]) -> Result<(), String> {
    let mut read = 0;
    while read < buf.len() {
        let n = stream.read(&mut buf[read..]).map_err(|e| e.to_string())?;
        if n == 0 { break; }
        read += n;
    }
    Ok(())
}
