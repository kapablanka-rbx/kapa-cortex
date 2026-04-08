use std::io::{BufRead, BufReader, Write};
use crate::iface::daemon_client;

/// Run the MCP stdio server. Reads JSON-RPC from stdin, writes to stdout.
pub fn run() {
    let stdin = std::io::stdin();
    let reader = BufReader::new(stdin.lock());
    let mut stdout = std::io::stdout().lock();

    for line in reader.lines() {
        let line = match line {
            Ok(l) if l.trim().is_empty() => continue,
            Ok(l) => l,
            Err(_) => break,
        };

        let response = handle_request(&line);
        writeln!(stdout, "{}", response).ok();
        stdout.flush().ok();
    }
}

fn handle_request(line: &str) -> String {
    let request: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return jsonrpc_error(serde_json::Value::Null, -32700, &e.to_string()),
    };

    let id = request.get("id").cloned().unwrap_or(serde_json::Value::Null);
    let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

    match method {
        "initialize" => jsonrpc_ok(&id, initialize_result()),
        "notifications/initialized" => return String::new(),
        "tools/list" => jsonrpc_ok(&id, tools_list()),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or_default();
            jsonrpc_ok(&id, handle_tool_call(&params))
        }
        _ => jsonrpc_error(id, -32601, &format!("Unknown method: {}", method)),
    }
}

fn initialize_result() -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "kapa-cortex", "version": "0.6.0" }
    })
}

fn tools_list() -> serde_json::Value {
    serde_json::json!({ "tools": build_tools() })
}

fn handle_tool_call(params: &serde_json::Value) -> serde_json::Value {
    let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or_default();

    let result = dispatch_tool(tool_name, &args);

    match result {
        Ok(data) => serde_json::json!({
            "content": [{"type": "text", "text": serde_json::to_string(&data).unwrap_or_default()}]
        }),
        Err(err) => serde_json::json!({
            "content": [{"type": "text", "text": err}],
            "isError": true
        }),
    }
}

fn dispatch_tool(name: &str, args: &serde_json::Value) -> Result<serde_json::Value, String> {
    match name {
        "defs" => {
            let symbol = require_str(args, "symbol")?;
            daemon_client::query("lookup", serde_json::json!({"target": symbol}))
        }
        "inspect" => {
            let symbol = require_str(args, "symbol")?;
            daemon_client::query("explain", serde_json::json!({"target": symbol}))
        }
        "refs" => {
            let symbol = require_str(args, "symbol")?;
            daemon_client::query("refs", serde_json::json!({"target": symbol}))
        }
        "rdeps" => {
            let target = require_str(args, "target")?;
            daemon_client::query("impact", serde_json::json!({"target": target}))
        }
        "deps" => {
            let target = require_str(args, "target")?;
            daemon_client::query("deps", serde_json::json!({"target": target}))
        }
        "trace" => {
            let source = require_str(args, "source")?;
            let target = require_str(args, "target")?;
            daemon_client::query("trace", serde_json::json!({"source": source, "target": target}))
        }
        "symbols" => {
            let file = require_str(args, "file")?;
            daemon_client::query("symbols", serde_json::json!({"target": file}))
        }
        "hotspots" => {
            let limit = args.get("limit").and_then(|l| l.as_u64()).unwrap_or(20);
            daemon_client::query("hotspots", serde_json::json!({"limit": limit}))
        }
        "status" => {
            daemon_client::query("status", serde_json::json!({}))
        }
        _ => Err(format!("Unknown tool: {}", name)),
    }
}

fn require_str<'a>(args: &'a serde_json::Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("Missing required parameter: {}", key))
}

fn build_tools() -> Vec<serde_json::Value> {
    vec![
        tool("defs", "Find all definitions of a symbol, scoped by class/namespace", serde_json::json!({
            "type": "object",
            "properties": { "symbol": {"type": "string", "description": "Symbol name (bare or FQN like MyClass::method)"} },
            "required": ["symbol"]
        })),
        tool("inspect", "Inspect a symbol: signature, callers, callees, overrides", serde_json::json!({
            "type": "object",
            "properties": { "symbol": {"type": "string", "description": "Symbol name"} },
            "required": ["symbol"]
        })),
        tool("refs", "Find all references to a symbol via LSP", serde_json::json!({
            "type": "object",
            "properties": { "symbol": {"type": "string", "description": "Symbol name"} },
            "required": ["symbol"]
        })),
        tool("rdeps", "Reverse dependencies: what files or callers break if this target changes", serde_json::json!({
            "type": "object",
            "properties": { "target": {"type": "string", "description": "File path or symbol name"} },
            "required": ["target"]
        })),
        tool("deps", "Forward dependencies: what this file imports/includes", serde_json::json!({
            "type": "object",
            "properties": { "target": {"type": "string", "description": "File path"} },
            "required": ["target"]
        })),
        tool("trace", "Trace the call path between two symbols", serde_json::json!({
            "type": "object",
            "properties": {
                "source": {"type": "string", "description": "Source symbol"},
                "target": {"type": "string", "description": "Target symbol"}
            },
            "required": ["source", "target"]
        })),
        tool("symbols", "List all symbols defined in a file", serde_json::json!({
            "type": "object",
            "properties": { "file": {"type": "string", "description": "File path"} },
            "required": ["file"]
        })),
        tool("hotspots", "Rank files by risk (complexity * dependents)", serde_json::json!({
            "type": "object",
            "properties": { "limit": {"type": "integer", "description": "Max results (default 20)"} }
        })),
        tool("status", "Index stats and daemon health", serde_json::json!({
            "type": "object", "properties": {}
        })),
    ]
}

fn tool(name: &str, description: &str, schema: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "description": description,
        "inputSchema": schema
    })
}

fn jsonrpc_ok(id: &serde_json::Value, result: serde_json::Value) -> String {
    serde_json::to_string(&serde_json::json!({
        "jsonrpc": "2.0", "id": id, "result": result
    })).unwrap_or_default()
}

fn jsonrpc_error(id: serde_json::Value, code: i64, message: &str) -> String {
    serde_json::to_string(&serde_json::json!({
        "jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}
    })).unwrap_or_default()
}
