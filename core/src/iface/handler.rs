use rusqlite::{Connection, params};
use std::os::unix::net::UnixStream;
use crate::domain::model::{ExplainResult, TraceResult, Reference};
use crate::infrastructure::sqlite;
use crate::infrastructure::lsp;
use crate::iface::server::DaemonState;
use super::protocol::{read_request, write_response, Response};

pub fn handle_connection(
    mut stream: UnixStream,
    state: &DaemonState,
) -> std::io::Result<()> {
    let request = read_request(&mut stream)?;
    let response = state.db.with_conn(|conn| {
        dispatch(&request.action, &request.params, conn, Some(&state.lsp_clients))
    });
    write_response(&mut stream, &response)
}

fn dispatch(
    action: &str,
    params: &serde_json::Value,
    conn: &Connection,
    lsp_lock: Option<&std::sync::Mutex<std::collections::HashMap<String, lsp::LspClient>>>,
) -> Response {
    let result = match action {
        "lookup" => handle_lookup(params, conn),
        "symbols" => handle_symbols(params, conn),
        "explain" => handle_explain(params, conn),
        "trace" => handle_trace(params, conn),
        "impact" => handle_impact(params, conn),
        "deps" => handle_deps(params, conn),
        "hotspots" => handle_hotspots(params, conn),
        "calls" => handle_calls(params, conn),
        "refs" => handle_refs(params, conn, lsp_lock),
        "reindex" => handle_reindex(params, conn),
        "status" => handle_status(conn, lsp_lock),
        _ => Err(format!("Unknown action: {}", action)),
    };

    match result {
        Ok(data) => Response::ok(data),
        Err(err) => Response::fail(&err),
    }
}

fn get_target(params: &serde_json::Value) -> Result<&str, String> {
    params
        .get("target")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing 'target' parameter".to_string())
}

fn handle_lookup(
    params: &serde_json::Value,
    conn: &Connection,
) -> Result<serde_json::Value, String> {
    let symbol = get_target(params)?;
    let defs = sqlite::lookup(conn, symbol).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "query": "lookup",
        "symbol": symbol,
        "definitions": defs,
    }))
}

fn handle_symbols(
    params: &serde_json::Value,
    conn: &Connection,
) -> Result<serde_json::Value, String> {
    let file_path = get_target(params)?;
    let symbols = sqlite::symbols_for_file(conn, file_path).map_err(|e| e.to_string())?;
    let total = symbols.len();
    Ok(serde_json::json!({
        "query": "symbols",
        "file": file_path,
        "symbols": symbols,
        "total": total,
    }))
}

fn handle_explain(
    params: &serde_json::Value,
    conn: &Connection,
) -> Result<serde_json::Value, String> {
    let fqn = get_target(params)?;
    let (scope, name) = split_fqn(fqn);

    let (file, line) = sqlite::find_scoped_definition(conn, name, scope)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Symbol not found: {}", fqn))?;

    let callers = sqlite::get_callers(conn, name, &file).map_err(|e| e.to_string())?;
    let callees = sqlite::get_callees(conn, name, &file).map_err(|e| e.to_string())?;

    let all_defs = sqlite::lookup(conn, name).map_err(|e| e.to_string())?;
    let overrides: Vec<_> = all_defs
        .into_iter()
        .filter(|d| {
            let d_scope = split_fqn(&d.fqn).0;
            d_scope != scope
        })
        .collect();

    let signature = read_line(&file, line as usize);

    let result = ExplainResult {
        fqn: fqn.to_string(),
        file,
        line,
        signature,
        callers,
        callees,
        overrides,
    };

    Ok(serde_json::json!({
        "query": "explain",
        "fqn": result.fqn,
        "file": result.file,
        "line": result.line,
        "signature": result.signature,
        "callers": result.callers,
        "callees": result.callees,
        "overrides": result.overrides,
    }))
}

fn handle_trace(
    params: &serde_json::Value,
    conn: &Connection,
) -> Result<serde_json::Value, String> {
    let source_fqn = params
        .get("source")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing 'source' parameter".to_string())?;
    let target_fqn = params
        .get("target")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing 'target' parameter".to_string())?;

    let (src_scope, src_name) = split_fqn(source_fqn);
    let (tgt_scope, tgt_name) = split_fqn(target_fqn);

    let (src_file, _) = sqlite::find_scoped_definition(conn, src_name, src_scope)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Source not found: {}", source_fqn))?;

    let (tgt_file, _) = sqlite::find_scoped_definition(conn, tgt_name, tgt_scope)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Target not found: {}", target_fqn))?;

    let path = sqlite::trace_path(conn, src_name, &src_file, tgt_name, &tgt_file)
        .map_err(|e| e.to_string())?;

    let result = TraceResult {
        source: source_fqn.to_string(),
        target: target_fqn.to_string(),
        path,
    };

    Ok(serde_json::json!({
        "query": "trace",
        "source": result.source,
        "target": result.target,
        "path": result.path,
        "hops": result.hops(),
    }))
}

fn handle_impact(
    params: &serde_json::Value,
    conn: &Connection,
) -> Result<serde_json::Value, String> {
    let target = get_target(params)?;

    // Check if target is a file or a symbol
    let file_exists: bool = conn
        .query_row("SELECT COUNT(*) FROM files WHERE path = ?", params![target], |row| row.get::<_, i64>(0))
        .map_err(|e| e.to_string())?
        > 0;

    if file_exists {
        let result = sqlite::find_impact(conn, target, 10).map_err(|e| e.to_string())?;
        Ok(serde_json::json!({
            "query": "impact",
            "target": target,
            "direct": result.direct,
            "transitive": result.transitive,
            "total_affected": result.total_affected(),
        }))
    } else {
        // Symbol — find its file, then do call impact
        let (scope, name) = split_fqn(target);
        let (file, _line) = sqlite::find_scoped_definition(conn, name, scope)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Not found: {}", target))?;

        let callers = sqlite::find_call_impact(conn, name, &file, 2).map_err(|e| e.to_string())?;
        let affected_files: Vec<String> = callers.iter().map(|c| c.file.clone()).collect::<std::collections::HashSet<_>>().into_iter().collect();
        Ok(serde_json::json!({
            "query": "symbol_impact",
            "target": target,
            "file": file,
            "callers": callers,
            "affected_files": affected_files,
            "total_affected": affected_files.len(),
        }))
    }
}

fn handle_deps(
    params: &serde_json::Value,
    conn: &Connection,
) -> Result<serde_json::Value, String> {
    let target = get_target(params)?;
    let deps = sqlite::find_deps(conn, target, 10).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "query": "deps",
        "target": target,
        "dependencies": deps,
        "total": deps.len(),
    }))
}

fn handle_hotspots(
    params: &serde_json::Value,
    conn: &Connection,
) -> Result<serde_json::Value, String> {
    let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
    let hotspots = sqlite::find_hotspots(conn, limit).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "query": "hotspots",
        "hotspots": hotspots,
    }))
}

fn handle_calls(
    params: &serde_json::Value,
    conn: &Connection,
) -> Result<serde_json::Value, String> {
    let fqn = get_target(params)?;
    let (scope, name) = split_fqn(fqn);

    let (file, _line) = sqlite::find_scoped_definition(conn, name, scope)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Symbol not found: {}", fqn))?;

    let callers = sqlite::find_call_impact(conn, name, &file, 10).map_err(|e| e.to_string())?;
    let affected_files: Vec<String> = callers.iter().map(|c| c.file.clone()).collect::<std::collections::HashSet<_>>().into_iter().collect();

    Ok(serde_json::json!({
        "query": "calls",
        "fqn": fqn,
        "file": file,
        "callers": callers,
        "affected_files": affected_files,
        "total_affected": affected_files.len(),
    }))
}

fn handle_refs(
    params: &serde_json::Value,
    conn: &Connection,
    lsp_lock: Option<&std::sync::Mutex<std::collections::HashMap<String, lsp::LspClient>>>,
) -> Result<serde_json::Value, String> {
    let fqn = get_target(params)?;
    let (scope, name) = split_fqn(fqn);

    let (file, line) = sqlite::find_scoped_definition(conn, name, scope)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Symbol not found: {}", fqn))?;

    let clients = lsp_lock
        .ok_or_else(|| "LSP not available".to_string())?
        .lock()
        .map_err(|e| e.to_string())?;

    // Pick the right LSP client based on file extension
    let ext = std::path::Path::new(&file).extension()
        .and_then(|e| e.to_str()).unwrap_or("");
    let lang = match ext {
        "c" | "h" | "cpp" | "cc" | "hpp" | "cxx" | "hxx" => "cpp",
        "py" | "pyi" => "python",
        "go" => "go",
        "rs" => "rust",
        "java" => "java",
        "lua" => "lua",
        "js" | "jsx" | "ts" | "tsx" => "typescript",
        _ => return Err(format!("No LSP for file type: {}", ext)),
    };

    let client = clients.get(lang)
        .ok_or_else(|| format!("No LSP running for {}", lang))?;

    let column = lsp::find_column(&file, line as usize, name) as i64;
    let raw_refs = client.get_references(&file, line - 1, column);

    let cwd_prefix = std::env::current_dir()
        .map(|p| format!("{}/", p.display()))
        .unwrap_or_default();

    let references: Vec<Reference> = raw_refs
        .iter()
        .filter_map(|loc| {
            let uri = loc.get("uri")?.as_str()?;
            let range = loc.get("range")?;
            let ref_line = range.get("start")?.get("line")?.as_i64()? + 1;
            let abs_path = uri.strip_prefix("file://")?;
            let rel_path = abs_path.strip_prefix(&cwd_prefix).unwrap_or(abs_path);
            Some(Reference { file: rel_path.to_string(), line: ref_line })
        })
        .collect();

    let total = references.len();
    Ok(serde_json::json!({
        "query": "refs",
        "fqn": fqn,
        "file": file,
        "line": line,
        "references": references,
        "total_references": total,
    }))
}

fn handle_reindex(
    params: &serde_json::Value,
    conn: &Connection,
) -> Result<serde_json::Value, String> {
    use crate::infrastructure::{ctags, hasher, complexity};
    use crate::application::indexer;

    let files: Vec<String> = if let Some(arr) = params.get("files").and_then(|f| f.as_array()) {
        arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
    } else {
        // Re-index all files in the DB
        let mut stmt = conn.prepare("SELECT path FROM files").map_err(|e| e.to_string())?;
        let rows: Vec<String> = stmt.query_map([], |row| row.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        rows
    };

    let mut reindexed = 0;
    for file_path in &files {
        if !std::path::Path::new(file_path).exists() {
            // File deleted — remove from index
            conn.execute("DELETE FROM symbols WHERE file_path = ?", params![file_path])
                .map_err(|e| e.to_string())?;
            conn.execute("DELETE FROM imports WHERE file_path = ?", params![file_path])
                .map_err(|e| e.to_string())?;
            conn.execute("DELETE FROM calls WHERE caller_file = ? OR callee_file = ?", params![file_path, file_path])
                .map_err(|e| e.to_string())?;
            conn.execute("DELETE FROM edges WHERE source = ? OR target = ?", params![file_path, file_path])
                .map_err(|e| e.to_string())?;
            conn.execute("DELETE FROM files WHERE path = ?", params![file_path])
                .map_err(|e| e.to_string())?;
            reindexed += 1;
            continue;
        }

        let hash = hasher::hash_file(file_path).map_err(|e| e.to_string())?;

        // Update file hash
        conn.execute(
            "INSERT OR REPLACE INTO files (path, content_hash) VALUES (?, ?)",
            params![file_path, hash],
        ).map_err(|e| e.to_string())?;

        // Clear old data for this file
        conn.execute("DELETE FROM symbols WHERE file_path = ?", params![file_path])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM imports WHERE file_path = ?", params![file_path])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM calls WHERE caller_file = ?", params![file_path])
            .map_err(|e| e.to_string())?;

        // Re-parse symbols
        if let Ok(symbols) = ctags::parse_file(file_path) {
            for sym in &symbols {
                conn.execute(
                    "INSERT INTO symbols (file_path, name, kind, line, scope) VALUES (?, ?, ?, ?, ?)",
                    params![file_path, sym.name, sym.kind, sym.line, sym.scope],
                ).map_err(|e| e.to_string())?;
            }
        }

        // Re-parse imports
        if let Ok(imports) = indexer::parse_imports_for_file(file_path) {
            for imp in &imports {
                conn.execute(
                    "INSERT INTO imports (file_path, raw, module, kind) VALUES (?, ?, ?, ?)",
                    params![file_path, imp.raw, imp.module, imp.kind],
                ).map_err(|e| e.to_string())?;
            }
        }

        // Update complexity
        if let Some(fc) = complexity::analyze_file(file_path) {
            conn.execute(
                "UPDATE files SET complexity = ?, lines = ? WHERE path = ?",
                params![fc.complexity, fc.lines, file_path],
            ).map_err(|e| e.to_string())?;
        }

        reindexed += 1;
    }

    Ok(serde_json::json!({"reindexed": reindexed}))
}

fn handle_status(
    conn: &Connection,
    lsp_lock: Option<&std::sync::Mutex<std::collections::HashMap<String, lsp::LspClient>>>,
) -> Result<serde_json::Value, String> {
    let files = sqlite::file_count(conn).map_err(|e| e.to_string())?;
    let symbols = sqlite::symbol_count(conn).map_err(|e| e.to_string())?;
    let edges = sqlite::edge_count(conn).map_err(|e| e.to_string())?;
    let calls = sqlite::call_count(conn).map_err(|e| e.to_string())?;
    let targets = sqlite::target_count(conn).map_err(|e| e.to_string())?;

    let lsp_list: Vec<serde_json::Value> = if let Some(lock) = lsp_lock {
        if let Ok(clients) = lock.lock() {
            clients.keys().map(|lang| {
                let server = lsp::server_binary(lang).unwrap_or("unknown");
                serde_json::json!({
                    "language": lang,
                    "server": server,
                    "status": "running",
                })
            }).collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    Ok(serde_json::json!({
        "daemon": true,
        "files": files,
        "symbols": symbols,
        "edges": edges,
        "calls": calls,
        "targets": targets,
        "lsp": lsp_list,
    }))
}

fn split_fqn(fqn: &str) -> (&str, &str) {
    if let Some(pos) = fqn.rfind("::") {
        (&fqn[..pos], &fqn[pos + 2..])
    } else {
        ("", fqn)
    }
}

fn read_line(file_path: &str, line: usize) -> String {
    std::fs::read_to_string(file_path)
        .ok()
        .and_then(|content| {
            content.lines().nth(line.saturating_sub(1)).map(|l| l.trim().to_string())
        })
        .unwrap_or_default()
}
