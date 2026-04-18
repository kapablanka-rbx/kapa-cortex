mod domain;
mod application;
mod infrastructure;
mod iface;

use clap::Parser;
use iface::cli::{Cli, Command, DaemonAction, Buck2Action, StackAction, OutputMode, parse_output_mode};
use std::path::PathBuf;
use std::sync::Arc;

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Daemon { action } => match action {
            DaemonAction::Start => start_daemon(),
            DaemonAction::Stop => stop_daemon(),
            DaemonAction::Status => daemon_status(),
        },
        Command::Index { root, clean } => {
            let root = root.as_deref().unwrap_or(".");
            if clean {
                let cache_dir = format!("{}/.cortex-cache", root);
                if std::path::Path::new(&cache_dir).exists() {
                    std::fs::remove_dir_all(&cache_dir).unwrap_or_else(|e| {
                        eprintln!("  \x1b[31mFailed to remove {}: {}\x1b[0m", cache_dir, e);
                        std::process::exit(1);
                    });
                    eprintln!("  \x1b[32m✓\x1b[0m Removed {}", cache_dir);
                }
            }
            run_index(root);
        }
        Command::Defs { symbol, json, brief } => query("lookup", serde_json::json!({"target": symbol}), parse_output_mode(json, brief)),
        Command::Refs { fqn, json, brief } => {
            let mode = parse_output_mode(json, brief);
            if fqn.len() == 1 {
                query("refs", serde_json::json!({"target": fqn[0]}), mode);
            } else {
                query("refs", serde_json::json!({"targets": fqn}), mode);
            }
        }
        Command::Inspect { fqn, json, brief } => query("explain", serde_json::json!({"target": fqn}), parse_output_mode(json, brief)),
        Command::Rdeps { target, json, brief } => query("impact", serde_json::json!({"target": target}), parse_output_mode(json, brief)),
        Command::Deps { target, json, brief } => query("deps", serde_json::json!({"target": target}), parse_output_mode(json, brief)),
        Command::Hotspots { limit, json, brief } => query("hotspots", serde_json::json!({"limit": limit}), parse_output_mode(json, brief)),
        Command::Symbols { file, json, brief } => query("symbols", serde_json::json!({"target": file}), parse_output_mode(json, brief)),
        Command::Trace { source, target, json, brief } => query("trace", serde_json::json!({"source": source, "target": target}), parse_output_mode(json, brief)),
        Command::Status => query("status", serde_json::json!({}), OutputMode::Json),
        Command::Reindex { files } => {
            if files.is_empty() {
                query("reindex", serde_json::json!({}), OutputMode::Text);
            } else {
                query("reindex", serde_json::json!({"files": files}), OutputMode::Text);
            }
        }
        Command::Analyze { base, max_files, max_lines, json, brief } => {
            let mode = parse_output_mode(json, brief);
            let base = base.unwrap_or_else(|| infrastructure::git::detect_base().unwrap_or("main".to_string()));
            match application::analyze::analyze_branch(&base, max_files, max_lines) {
                Ok(result) => match mode {
                    OutputMode::Json => iface::reporter::print_analysis_json(&result),
                    OutputMode::Briefing => iface::reporter::print_analysis_brief(&result),
                    OutputMode::Text => iface::reporter::print_analysis_text(&result),
                },
                Err(e) => {
                    eprintln!("  \x1b[31m{}\x1b[0m", e);
                    std::process::exit(1);
                }
            }
        }
        Command::Extract { description, base, branch, json, brief } => {
            let mode = parse_output_mode(json, brief);
            let base = base.unwrap_or_else(|| infrastructure::git::detect_base().unwrap_or("main".to_string()));
            match application::extract::extract_files(&base, &description) {
                Ok(result) => {
                    // Create branch if requested
                    if let Some(ref branch_name) = branch {
                        if result.matched_files.is_empty() {
                            eprintln!("  \x1b[33mNo files matched — branch not created\x1b[0m");
                        } else {
                            match application::extract::create_extraction_branch(&base, branch_name, &result.matched_files) {
                                Ok(()) => eprintln!("  \x1b[32m✓\x1b[0m Branch '{}' created with {} files", branch_name, result.matched_files.len()),
                                Err(e) => {
                                    eprintln!("  \x1b[31mBranch creation failed: {}\x1b[0m", e);
                                    std::process::exit(1);
                                }
                            }
                        }
                    }
                    match mode {
                        OutputMode::Json => println!("{}", serde_json::json!({
                            "description": result.description,
                            "matched": result.matched_files,
                            "unmatched": result.unmatched_files,
                            "total_matched": result.matched_files.len(),
                        })),
                        OutputMode::Briefing => {
                            println!("matched: {}", result.matched_files.len());
                            for f in &result.matched_files { println!("  {}", f); }
                        }
                        OutputMode::Text => {
                            println!("  \x1b[1mExtract:\x1b[0m {}\n", result.description);
                            println!("  \x1b[32mMatched ({}):\x1b[0m", result.matched_files.len());
                            for f in &result.matched_files { println!("    {}", f); }
                            if !result.unmatched_files.is_empty() {
                                println!("\n  \x1b[33mNot matched ({}):\x1b[0m", result.unmatched_files.len());
                                for f in &result.unmatched_files { println!("    {}", f); }
                            }
                        }
                    }
                },
                Err(e) => {
                    eprintln!("  \x1b[31m{}\x1b[0m", e);
                    std::process::exit(1);
                }
            }
        }
        Command::Owner { file, json, brief } => run_owner(&file, parse_output_mode(json, brief)),
        Command::Buck2 { action } => run_buck2(action),
        Command::Stack { action } => run_stack(action),
        Command::InstallSkill => run_install(),
    }
}

fn run_stack(action: StackAction) {
    match action {
        StackAction::Plan { base, max_files, max_lines } => {
            let base = base.unwrap_or_else(|| {
                infrastructure::git::detect_base().unwrap_or("main".to_string())
            });
            match application::stack::create_plan(&base, max_files, max_lines) {
                Ok(plan) => {
                    eprintln!("  \x1b[32m✓\x1b[0m Stack plan: {} PRs written to .cortex-cache/stack-plan.json", plan.prs.len());
                    for stack_pr in &plan.prs {
                        eprintln!("    #{} {} ({} files, risk: {})",
                            stack_pr.order, stack_pr.title,
                            stack_pr.files.len(), stack_pr.risk_level);
                    }
                }
                Err(error) => {
                    eprintln!("  \x1b[31mStack plan failed: {}\x1b[0m", error);
                    std::process::exit(1);
                }
            }
        }
        StackAction::Apply { plan, dry_run } => {
            match application::stack::apply_plan(plan.as_deref(), dry_run) {
                Ok(pr_urls) => {
                    if dry_run {
                        eprintln!("  \x1b[33mDry run complete — no changes made\x1b[0m");
                    } else {
                        eprintln!("  \x1b[32m✓\x1b[0m Created {} PRs", pr_urls.len());
                    }
                }
                Err(error) => {
                    eprintln!("  \x1b[31mStack apply failed: {}\x1b[0m", error);
                    std::process::exit(1);
                }
            }
        }
    }
}

fn run_install() {
    let skill_content = include_str!("../../.claude/skills/cortex/SKILL.md");

    for name in &["cortex", "kapa"] {
        let skill_dir = PathBuf::from(format!(".claude/skills/{}", name));
        std::fs::create_dir_all(&skill_dir).unwrap_or_else(|e| {
            eprintln!("  \x1b[31mFailed to create {}: {}\x1b[0m", skill_dir.display(), e);
            std::process::exit(1);
        });

        let skill_path = skill_dir.join("SKILL.md");
        std::fs::write(&skill_path, skill_content).unwrap_or_else(|e| {
            eprintln!("  \x1b[31mFailed to write {}: {}\x1b[0m", skill_path.display(), e);
            std::process::exit(1);
        });

        eprintln!("  \x1b[32m✓\x1b[0m Installed /{} → {}", name, skill_path.display());
    }
}

fn open_db() -> infrastructure::sqlite::Database {
    let db_path = PathBuf::from(".cortex-cache/index.db");
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    infrastructure::sqlite::Database::open(&db_path).unwrap_or_else(|e| {
        eprintln!("  \x1b[31mFailed to open database: {}\x1b[0m", e);
        std::process::exit(1);
    })
}

fn start_daemon() {
    let db = Arc::new(open_db());
    db.with_conn(|conn| {
        if let (Ok(files), Ok(symbols), Ok(edges), Ok(calls)) = (
            infrastructure::sqlite::file_count(conn),
            infrastructure::sqlite::symbol_count(conn),
            infrastructure::sqlite::edge_count(conn),
            infrastructure::sqlite::call_count(conn),
        ) {
            eprintln!("  \x1b[32m✓\x1b[0m Index: {} files, {} symbols, {} edges, {} calls", files, symbols, edges, calls);
        }
    });
    if let Err(err) = iface::server::run(db) {
        eprintln!("  \x1b[31mServer error: {}\x1b[0m", err);
        std::process::exit(1);
    }
}

fn check_tools() {
    let required = [
        ("ctags", "brew install universal-ctags  /  apt install universal-ctags  /  dnf install ctags"),
    ];
    let optional = [
        ("lizard", "pip install lizard  (complexity metrics will be skipped without it)"),
    ];
    for (tool, hint) in &required {
        let found = std::process::Command::new("which").arg(tool).output()
            .map(|output| output.status.success())
            .unwrap_or(false);
        if found {
            eprintln!("  \x1b[32m✓\x1b[0m {}", tool);
        } else {
            eprintln!("  \x1b[31m✗\x1b[0m {} not found — {}", tool, hint);
            std::process::exit(1);
        }
    }
    for (tool, hint) in &optional {
        let found = std::process::Command::new("which").arg(tool).output()
            .map(|output| output.status.success())
            .unwrap_or(false);
        if found {
            eprintln!("  \x1b[32m✓\x1b[0m {}", tool);
        } else {
            eprintln!("  \x1b[33m⚠\x1b[0m {} not found — {}", tool, hint);
        }
    }
}

fn run_index(root: &str) {
    eprintln!("  \x1b[36m→ Checking tools...\x1b[0m");
    check_tools();

    let db_path = PathBuf::from(format!("{}/.cortex-cache/index.db", root));
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let db = infrastructure::sqlite::Database::open(&db_path).unwrap_or_else(|e| {
        eprintln!("  \x1b[31mFailed to open database: {}\x1b[0m", e);
        std::process::exit(1);
    });
    db.with_conn(|conn| {
        conn.execute_batch("DELETE FROM files; DELETE FROM symbols; DELETE FROM imports; DELETE FROM edges; DELETE FROM calls;").ok();
    });
    if let Err(e) = application::indexer::index_repo(&db, root) {
        eprintln!("  \x1b[31mIndex error: {}\x1b[0m", e);
        std::process::exit(1);
    }
}

fn ensure_daemon() {
    use std::os::unix::net::UnixStream;

    // Already running?
    if UnixStream::connect(iface::server::SOCKET_PATH).is_ok() {
        return;
    }

    // Check if index exists, build if not
    let db_path = PathBuf::from(".cortex-cache/index.db");
    let needs_index = !db_path.exists() || {
        let db = infrastructure::sqlite::Database::open(&db_path).ok();
        db.map(|d| d.with_conn(|c| infrastructure::sqlite::file_count(c).unwrap_or(0) == 0)).unwrap_or(true)
    };

    if needs_index {
        run_index(".");
    }

    // Start daemon in background
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("kapa-cortex"));
    let child = std::process::Command::new(&exe)
        .args(["daemon", "start"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::inherit())
        .spawn();

    if child.is_err() {
        eprintln!("  \x1b[31mFailed to start daemon\x1b[0m");
        std::process::exit(1);
    }

    // Wait for socket to be ready
    for _ in 0..100 {
        if UnixStream::connect(iface::server::SOCKET_PATH).is_ok() {
            std::thread::sleep(std::time::Duration::from_millis(500));
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    eprintln!("  \x1b[33mDaemon started but socket not ready\x1b[0m");
}

fn query(action: &str, params: serde_json::Value, mode: OutputMode) {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    ensure_daemon();

    // Retry connection — daemon may need a moment after socket bind
    let mut stream = None;
    for _ in 0..10 {
        match UnixStream::connect(iface::server::SOCKET_PATH) {
            Ok(s) => { stream = Some(s); break; }
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(200)),
        }
    }
    let mut stream = match stream {
        Some(s) => s,
        None => {
            eprintln!("  \x1b[31mDaemon not responding\x1b[0m");
            std::process::exit(1);
        }
    };

    let payload = serde_json::json!({"action": action, "params": params});
    let bytes = serde_json::to_vec(&payload).unwrap();
    let header = (bytes.len() as u64).to_be_bytes();
    stream.write_all(&header).unwrap();
    stream.write_all(&bytes).unwrap();

    let mut header_buf = [0u8; 8];
    stream.read_exact(&mut header_buf).unwrap();
    let length = u64::from_be_bytes(header_buf) as usize;
    let mut response = vec![0u8; length];
    let mut read = 0;
    while read < length {
        let n = stream.read(&mut response[read..]).unwrap();
        if n == 0 { break; }
        read += n;
    }

    let parsed: serde_json::Value = serde_json::from_slice(&response).unwrap_or_default();
    if parsed.get("status").and_then(|s| s.as_str()) == Some("error") {
        let error = parsed.get("error").and_then(|e| e.as_str()).unwrap_or("unknown error");
        eprintln!("  \x1b[31m{}\x1b[0m", error);
        std::process::exit(1);
    }
    let data = parsed.get("data").unwrap_or(&serde_json::Value::Null);
    match mode {
        OutputMode::Json => println!("{}", serde_json::to_string_pretty(data).unwrap_or_default()),
        OutputMode::Briefing => print_briefing(action, data),
        OutputMode::Text => print_result(action, data),
    }
}

fn print_result(action: &str, data: &serde_json::Value) {
    match action {
        "lookup" => {
            let symbol = data.get("symbol").and_then(|s| s.as_str()).unwrap_or("");
            let defs = data.get("definitions").and_then(|d| d.as_array());
            if let Some(defs) = defs {
                println!("  \x1b[1m{}\x1b[0m ({} definitions):", symbol, defs.len());
                for d in defs {
                    let fqn = d.get("fqn").and_then(|s| s.as_str()).unwrap_or("");
                    let kind = d.get("kind").and_then(|s| s.as_str()).unwrap_or("");
                    let file = d.get("file").and_then(|s| s.as_str()).unwrap_or("");
                    let line = d.get("line").and_then(|l| l.as_i64()).unwrap_or(0);
                    println!("    {:<50} {:<10} {}:{}", fqn, kind, file, line);
                }
            }
        }
        "impact" | "symbol_impact" => {
            let target = data.get("target").and_then(|s| s.as_str()).unwrap_or("");
            let total = data.get("total_affected").and_then(|n| n.as_i64()).unwrap_or(0);
            println!("  \x1b[1mImpact of {}\x1b[0m ({} affected):", target, total);
            if let Some(direct) = data.get("direct").and_then(|d| d.as_array()) {
                for d in direct { println!("    {}  direct", d.as_str().unwrap_or("")); }
            }
            if let Some(transitive) = data.get("transitive").and_then(|d| d.as_array()) {
                for t in transitive.iter().take(20) { println!("    {}  transitive", t.as_str().unwrap_or("")); }
                if transitive.len() > 20 { println!("    ... and {} more", transitive.len() - 20); }
            }
        }
        "deps" => {
            let target = data.get("target").and_then(|s| s.as_str()).unwrap_or("");
            let total = data.get("total").and_then(|n| n.as_i64()).unwrap_or(0);
            println!("  \x1b[1mDependencies of {}\x1b[0m ({}):", target, total);
            if let Some(deps) = data.get("dependencies").and_then(|d| d.as_array()) {
                for d in deps { println!("    {}", d.as_str().unwrap_or("")); }
            }
        }
        "hotspots" => {
            if let Some(hotspots) = data.get("hotspots").and_then(|h| h.as_array()) {
                for h in hotspots {
                    let path = h.get("path").and_then(|s| s.as_str()).unwrap_or("");
                    let cx = h.get("complexity").and_then(|n| n.as_i64()).unwrap_or(0);
                    let deps = h.get("dependents").and_then(|n| n.as_i64()).unwrap_or(0);
                    let score = h.get("score").and_then(|n| n.as_f64()).unwrap_or(0.0);
                    println!("    {:<60} c={} d={} s={:.0}", path, cx, deps, score);
                }
            }
        }
        "explain" => {
            let fqn = data.get("fqn").and_then(|s| s.as_str()).unwrap_or("");
            let sig = data.get("signature").and_then(|s| s.as_str()).unwrap_or("");
            let file = data.get("file").and_then(|s| s.as_str()).unwrap_or("");
            let line = data.get("line").and_then(|l| l.as_i64()).unwrap_or(0);
            println!("  \x1b[1m{}\x1b[0m\n  \x1b[2m{}\x1b[0m\n  \x1b[2m{}:{}\x1b[0m", fqn, sig, file, line);
            for section in &["callers", "callees", "overrides"] {
                if let Some(items) = data.get(*section).and_then(|c| c.as_array()) {
                    if !items.is_empty() {
                        println!("  {} ({}):", section, items.len());
                        for item in items {
                            let f = item.get("function").or(item.get("fqn")).and_then(|s| s.as_str()).unwrap_or("");
                            let file = item.get("file").and_then(|s| s.as_str()).unwrap_or("");
                            let line = item.get("line").and_then(|l| l.as_i64()).unwrap_or(0);
                            println!("    {}  {}:{}", f, file, line);
                        }
                    }
                }
            }
        }
        "symbols" => {
            let file = data.get("file").and_then(|s| s.as_str()).unwrap_or("");
            let total = data.get("total").and_then(|n| n.as_i64()).unwrap_or(0);
            println!("  \x1b[1m{}\x1b[0m ({} symbols):", file, total);
            if let Some(symbols) = data.get("symbols").and_then(|s| s.as_array()) {
                for s in symbols {
                    let name = s.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let kind = s.get("kind").and_then(|k| k.as_str()).unwrap_or("");
                    let line = s.get("line").and_then(|l| l.as_i64()).unwrap_or(0);
                    let scope = s.get("scope").and_then(|s| s.as_str()).unwrap_or("");
                    if scope.is_empty() { println!("    {:>5} {:<10} {}", line, kind, name); }
                    else { println!("    {:>5} {:<10} {}::{}", line, kind, scope, name); }
                }
            }
        }
        "refs" => {
            let fqn = data.get("fqn").and_then(|s| s.as_str()).unwrap_or("");
            let total = data.get("total_references").and_then(|n| n.as_i64()).unwrap_or(0);
            println!("  \x1b[1m{}\x1b[0m ({} references):", fqn, total);
            if let Some(refs) = data.get("references").and_then(|r| r.as_array()) {
                let mut by_file: std::collections::BTreeMap<String, Vec<i64>> = std::collections::BTreeMap::new();
                for r in refs {
                    let file = r.get("file").and_then(|s| s.as_str()).unwrap_or("").to_string();
                    let line = r.get("line").and_then(|l| l.as_i64()).unwrap_or(0);
                    by_file.entry(file).or_default().push(line);
                }
                for (file, lines) in &by_file {
                    let line_str: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
                    println!("    {} :{}", file, line_str.join(","));
                }
            }
        }
        "trace" => {
            let source = data.get("source").and_then(|s| s.as_str()).unwrap_or("");
            let target = data.get("target").and_then(|s| s.as_str()).unwrap_or("");
            let hops = data.get("hops").and_then(|n| n.as_i64()).unwrap_or(0);
            println!("  \x1b[1m{} → {}\x1b[0m ({} hops):", source, target, hops);
            if let Some(path) = data.get("path").and_then(|p| p.as_array()) {
                for step in path {
                    let f = step.get("function").and_then(|s| s.as_str()).unwrap_or("");
                    let file = step.get("file").and_then(|s| s.as_str()).unwrap_or("");
                    let line = step.get("line").and_then(|l| l.as_i64()).unwrap_or(0);
                    println!("    → {}  {}:{}", f, file, line);
                }
            }
        }
        _ => { println!("{}", serde_json::to_string_pretty(data).unwrap_or_default()); }
    }
}

fn js(v: &serde_json::Value, k: &str) -> String { v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string() }
fn jn(v: &serde_json::Value, k: &str) -> i64 { v.get(k).and_then(|x| x.as_i64()).unwrap_or(0) }
fn ja<'a>(v: &'a serde_json::Value, k: &str) -> Option<&'a Vec<serde_json::Value>> { v.get(k).and_then(|x| x.as_array()) }

fn print_briefing(action: &str, data: &serde_json::Value) {
    match action {
        "explain" => {
            println!("symbol: {}", js(data, "fqn"));
            let sig = js(data, "signature");
            if !sig.is_empty() { println!("sig: {}", sig); }
            println!("def: {}:{}", js(data, "file"), jn(data, "line"));
            let cap = 10;
            for section in &["callers", "callees", "overrides"] {
                if let Some(items) = ja(data, section) {
                    if !items.is_empty() {
                        println!("{}: {}", section, items.len());
                        for item in items.iter().take(cap) {
                            let f = item.get("function").or(item.get("fqn")).and_then(|x| x.as_str()).unwrap_or("");
                            println!("  {} {}:{}", f, js(item, "file"), jn(item, "line"));
                        }
                        if items.len() > cap {
                            println!("  +{} more", items.len() - cap);
                        }
                    }
                }
            }
        }
        "lookup" => {
            if let Some(defs) = ja(data, "definitions") {
                let cap = 20;
                println!("definitions: {}", defs.len());
                for d in defs.iter().take(cap) {
                    println!("  {} {} {}:{}", js(d, "fqn"), js(d, "kind"), js(d, "file"), jn(d, "line"));
                }
                if defs.len() > cap { println!("  +{} more", defs.len() - cap); }
            }
        }
        "refs" => {
            println!("symbol: {} {}:{}", js(data, "fqn"), js(data, "file"), jn(data, "line"));
            if let Some(refs) = ja(data, "references") {
                println!("refs: {}", refs.len());
                // Group by file to reduce output size
                let mut by_file: std::collections::BTreeMap<String, Vec<i64>> = std::collections::BTreeMap::new();
                for r in refs {
                    by_file.entry(js(r, "file")).or_default().push(jn(r, "line"));
                }
                for (file, lines) in &by_file {
                    let line_str: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
                    println!("  {} :{}", file, line_str.join(","));
                }
            }
        }
        "impact" | "symbol_impact" => {
            let target = js(data, "target");
            let total = jn(data, "total_affected");
            let risk = if total <= 2 { "low" } else if total <= 10 { "medium" } else { "high" };
            println!("target: {}", target);
            let file = js(data, "file");
            if !file.is_empty() { println!("def: {}", file); }
            println!("risk: {} ({} affected)", risk, total);

            if let Some(callers) = ja(data, "callers") {
                let mut seen = std::collections::HashSet::new();
                println!("callers:");
                for c in callers {
                    let key = format!("{} {}", js(c, "function"), js(c, "file"));
                    if seen.insert(key) {
                        println!("  {} {}:{}", js(c, "function"), js(c, "file"), jn(c, "line"));
                    }
                }
                let mut files: Vec<String> = callers.iter()
                    .map(|c| js(c, "file")).collect::<std::collections::HashSet<_>>()
                    .into_iter().collect();
                files.sort();
                println!("blast: {}", files.join(", "));
                return;
            }
            if let Some(direct) = ja(data, "direct") {
                println!("direct: {} files", direct.len());
                for f in direct.iter().take(20) { println!("  {}", f.as_str().unwrap_or("")); }
                if direct.len() > 20 { println!("  +{} more", direct.len() - 20); }
            }
            if let Some(trans) = ja(data, "transitive") {
                if !trans.is_empty() { println!("transitive: {} files", trans.len()); }
            }
        }
        "deps" => {
            println!("file: {}", js(data, "target"));
            println!("deps: {}", jn(data, "total"));
            if let Some(deps) = ja(data, "dependencies") {
                for d in deps { println!("  {}", d.as_str().unwrap_or("")); }
            }
        }
        "hotspots" => {
            if let Some(hotspots) = ja(data, "hotspots") {
                for h in hotspots {
                    println!("{} complexity={} dependents={} score={:.0}",
                        js(h, "path"), jn(h, "complexity"), jn(h, "dependents"),
                        h.get("score").and_then(|n| n.as_f64()).unwrap_or(0.0));
                }
            }
        }
        "symbols" => {
            println!("file: {}", js(data, "file"));
            if let Some(symbols) = ja(data, "symbols") {
                for sym in symbols {
                    let kind = sym.get("kind").and_then(|k| k.as_str()).unwrap_or("");
                    if kind == "local" || kind == "parameter" { continue; }
                    let scope = js(sym, "scope");
                    let name = js(sym, "name");
                    let fqn = if scope.is_empty() { name } else { format!("{}::{}", scope, name) };
                    println!("  {} {} {}", jn(sym, "line"), kind, fqn);
                }
            }
        }
        "trace" => {
            let hops = jn(data, "hops");
            println!("{} -> {} ({} hops)", js(data, "source"), js(data, "target"), hops);
            if let Some(path) = ja(data, "path") {
                for step in path {
                    println!("  -> {} {}:{}", js(step, "function"), js(step, "file"), jn(step, "line"));
                }
            }
        }
        "status" => {
            println!("files={} symbols={} calls={}", jn(data, "files"), jn(data, "symbols"), jn(data, "calls"));
            if let Some(lsp) = ja(data, "lsp") {
                for s in lsp { println!("lsp: {} {}", js(s, "language"), js(s, "status")); }
            }
        }
        _ => { println!("{}", serde_json::to_string_pretty(data).unwrap_or_default()); }
    }
}

fn run_owner(file: &str, mode: OutputMode) {
    let db = open_db();
    let buck_results = db.with_conn(|conn| {
        infrastructure::sqlite::find_target_for_file(conn, file).unwrap_or_default()
    });
    let cmake_results = infrastructure::cmake::find_cmake_owner(file);
    let has_any = !buck_results.is_empty() || !cmake_results.is_empty();

    match mode {
        OutputMode::Json => {
            let buck_json: Vec<_> = buck_results.iter().map(|t| {
                let pkg = infrastructure::buck2::package_from_targets_path(&t.path);
                serde_json::json!({"label": format!("//{}:{}", pkg, t.name), "rule": t.rule, "deps": t.deps})
            }).collect();
            let cmake_json: Vec<_> = cmake_results.iter().map(|c| {
                serde_json::json!({"target": c.target_name, "rule": c.rule, "cmake_file": c.cmake_file})
            }).collect();
            println!("{}", serde_json::json!({"file": file, "buck2": buck_json, "cmake": cmake_json}));
        }
        OutputMode::Briefing => {
            if !has_any {
                println!("owner: none found for {}", file);
            } else {
                println!("file: {}", file);
                for t in &buck_results {
                    let pkg = infrastructure::buck2::package_from_targets_path(&t.path);
                    println!("  [buck2] //{}:{} {}", pkg, t.name, t.rule);
                }
                for c in &cmake_results {
                    println!("  [cmake] {} ({}) in {}", c.target_name, c.rule, c.cmake_file);
                }
            }
        }
        OutputMode::Text => {
            if !has_any {
                println!("  No target found owning {}", file);
            } else {
                println!("  \x1b[1mOwners of {}\x1b[0m\n", file);
                for t in &buck_results {
                    let pkg = infrastructure::buck2::package_from_targets_path(&t.path);
                    println!("  \x1b[36m[buck2]\x1b[0m //{}:{}  ({})", pkg, t.name, t.rule);
                    if let Some(ref deps) = t.deps {
                        if deps != "[]" && !deps.is_empty() {
                            println!("    deps: {}", deps);
                        }
                    }
                }
                for c in &cmake_results {
                    println!("  \x1b[35m[cmake]\x1b[0m {}  ({})  — {}", c.target_name, c.rule, c.cmake_file);
                }
            }
        }
    }
}

fn run_buck2(action: Buck2Action) {
    let db = open_db();
    db.with_conn(|conn| {
        match action {
            Buck2Action::Targets { rule, brief } => {
                let query = if let Some(ref r) = rule {
                    format!("SELECT path, name, rule FROM targets WHERE rule = '{}' ORDER BY path, name", r)
                } else {
                    "SELECT path, name, rule FROM targets ORDER BY path, name".to_string()
                };
                let mut stmt = conn.prepare(&query).unwrap();
                let rows: Vec<(String, String, String)> = stmt
                    .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap();

                if brief {
                    println!("targets: {}", rows.len());
                    for (path, name, rule) in &rows {
                        let pkg = infrastructure::buck2::package_from_targets_path(path);
                        println!("  //{}:{} {}", pkg, name, rule);
                    }
                } else {
                    println!("  \x1b[1m{} targets\x1b[0m\n", rows.len());
                    for (path, name, rule) in &rows {
                        let pkg = infrastructure::buck2::package_from_targets_path(path);
                        println!("  //{}:{:<40} {}", pkg, name, rule);
                    }
                }
            }
            Buck2Action::Deps { label, brief } => {
                let deps = infrastructure::sqlite::target_deps(conn, &label).unwrap();
                if brief {
                    println!("target: {}", label);
                    println!("deps: {}", deps.len());
                    for d in &deps { println!("  {}", d); }
                } else {
                    println!("  \x1b[1mDeps of {}\x1b[0m ({})\n", label, deps.len());
                    for d in &deps { println!("  {}", d); }
                }
            }
            Buck2Action::Rdeps { label, brief } => {
                let rdeps = infrastructure::sqlite::target_rdeps(conn, &label).unwrap();
                if brief {
                    println!("target: {}", label);
                    println!("rdeps: {}", rdeps.len());
                    for r in &rdeps { println!("  {}", r); }
                } else {
                    println!("  \x1b[1mReverse deps of {}\x1b[0m ({})\n", label, rdeps.len());
                    for r in &rdeps { println!("  {}", r); }
                }
            }
        }
    });
}

fn stop_daemon() {
    let pid_str = match std::fs::read_to_string(iface::server::PID_PATH) {
        Ok(s) => s.trim().to_string(),
        Err(_) => {
            eprintln!("  \x1b[33mNo daemon running (no PID file).\x1b[0m");
            return;
        }
    };
    let pid: u32 = match pid_str.parse() {
        Ok(p) => p,
        Err(_) => {
            eprintln!("  \x1b[31mInvalid PID file.\x1b[0m");
            return;
        }
    };
    let result = std::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status();
    match result {
        Ok(status) if status.success() => {
            std::fs::remove_file(iface::server::PID_PATH).ok();
            std::fs::remove_file(iface::server::SOCKET_PATH).ok();
            eprintln!("  \x1b[32m✓\x1b[0m Daemon stopped.");
        }
        _ => eprintln!("  \x1b[33mDaemon not running (stale PID {}).\x1b[0m", pid),
    }
}

fn daemon_status() {
    query("status", serde_json::json!({}), OutputMode::Json);
}

