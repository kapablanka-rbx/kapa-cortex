use crate::infrastructure::sqlite::Database;
use crate::infrastructure::{ctags, hasher, walker};
use crate::infrastructure::tree_sitter as ts;
use rusqlite::params;
use std::time::Instant;

/// Full index: walk repo, parse each file, build edges.
pub fn index_repo(db: &Database, root: &str) -> Result<(), String> {
    let start = Instant::now();
    let files = walker::find_source_files(root)?;
    let total = files.len();
    eprintln!("  \x1b[36mIndexing {} files...\x1b[0m", total);

    let root_prefix = format!(
        "{}/",
        std::fs::canonicalize(root).map_err(|e| e.to_string())?.display()
    );

    db.with_conn(|conn| -> Result<(), String> {
        conn.execute_batch("BEGIN").map_err(|e| e.to_string())?;

        let mut symbol_count: usize = 0;
        let mut import_count: usize = 0;
        let mut call_count: usize = 0;

        for (idx, file_path) in files.iter().enumerate() {
            if idx % 100 == 0 && idx > 0 {
                let elapsed = start.elapsed().as_secs();
                let pct = idx * 100 / total;
                eprint!("\r\x1b[2K  \x1b[36m{}/{} ({}%) {}s\x1b[0m", idx, total, pct, elapsed);
            }

            let abs = std::fs::canonicalize(file_path)
                .unwrap_or_else(|_| std::path::PathBuf::from(file_path));
            let relative = abs
                .to_string_lossy()
                .strip_prefix(&root_prefix)
                .unwrap_or(&abs.to_string_lossy())
                .to_string();

            let hash = hasher::hash_file(file_path)?;

            conn.execute(
                "INSERT OR REPLACE INTO files (path, content_hash) VALUES (?, ?)",
                params![relative, hash],
            )
            .map_err(|e| e.to_string())?;

            let symbols = ctags::parse_file(file_path)?;
            for sym in &symbols {
                conn.execute(
                    "INSERT INTO symbols (file_path, name, kind, line, scope) VALUES (?, ?, ?, ?, ?)",
                    params![relative, sym.name, sym.kind, sym.line, sym.scope],
                )
                .map_err(|e| e.to_string())?;
                symbol_count += 1;
            }

            let file_imports = parse_imports(file_path)?;
            for imp in &file_imports {
                conn.execute(
                    "INSERT INTO imports (file_path, raw, module, kind) VALUES (?, ?, ?, ?)",
                    params![relative, imp.raw, imp.module, imp.kind],
                )
                .map_err(|e| e.to_string())?;
                import_count += 1;
            }

            let lang = detect_language(file_path);
            if let Some(lang_name) = lang {
                let source =
                    String::from_utf8_lossy(&std::fs::read(file_path).unwrap_or_default())
                        .to_string();
                let call_sites = ts::extract_calls(&source, lang_name);
                for call in &call_sites {
                    conn.execute(
                        "INSERT INTO calls (caller_file, caller_function, callee_file, callee_function, line)
                         VALUES (?, ?, '', ?, ?)",
                        params![relative, call.caller_function, call.callee_name, call.line],
                    )
                    .map_err(|e| e.to_string())?;
                    call_count += 1;
                }
            }
        }

        conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;

        // Resolve callee_file: tree-sitter only gives us the callee name,
        // look up the actual file from the symbols table.
        let resolved = conn.execute(
            "UPDATE calls SET callee_file = COALESCE((
                SELECT s.file_path FROM symbols s
                WHERE s.name = calls.callee_function
                  AND s.kind IN ('function', 'method', 'member')
                LIMIT 1
            ), '') WHERE callee_file = ''",
            [],
        ).map_err(|e| e.to_string())?;

        let elapsed = start.elapsed().as_secs_f32();
        eprintln!(
            "\r\x1b[2K  \x1b[32m✓\x1b[0m {} symbols, {} imports, {} calls ({} resolved) ({:.1}s)",
            symbol_count, import_count, call_count, resolved, elapsed
        );
        Ok(())
    })?;

    let edge_start = Instant::now();
    let edge_count = build_edges(db)?;
    eprintln!(
        "  \x1b[32m✓\x1b[0m {} edges ({:.1}s)",
        edge_count,
        edge_start.elapsed().as_secs_f32()
    );
    // Complexity via lizard
    let cx_start = Instant::now();
    let cx_count = compute_complexity(db, root, &root_prefix)?;
    eprintln!(
        "  \x1b[32m✓\x1b[0m {} files with complexity ({:.1}s)",
        cx_count,
        cx_start.elapsed().as_secs_f32()
    );

    // Buck2 targets
    let buck_start = Instant::now();
    let target_count = index_targets(db, root, &root_prefix)?;
    if target_count > 0 {
        eprintln!(
            "  \x1b[32m✓\x1b[0m {} targets ({:.1}s)",
            target_count,
            buck_start.elapsed().as_secs_f32()
        );
    }

    eprintln!(
        "  \x1b[32m✓\x1b[0m Index complete in {:.1}s",
        start.elapsed().as_secs_f32()
    );
    Ok(())
}

fn index_targets(db: &Database, root: &str, root_prefix: &str) -> Result<usize, String> {
    use crate::infrastructure::buck2;

    let buck_files = walker::find_buck_files(root)?;
    if buck_files.is_empty() { return Ok(0); }

    let mut count: usize = 0;
    db.with_conn(|conn| -> Result<(), String> {
        conn.execute_batch("BEGIN").map_err(|e| e.to_string())?;

        for file_path in &buck_files {
            let abs = std::fs::canonicalize(file_path)
                .unwrap_or_else(|_| std::path::PathBuf::from(file_path));
            let relative = abs
                .to_string_lossy()
                .strip_prefix(root_prefix)
                .unwrap_or(&abs.to_string_lossy())
                .to_string();

            let source = match std::fs::read_to_string(file_path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let parsed = match buck2::parse_targets_file(&relative, &source) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("  \x1b[33mSkipping {}: {}\x1b[0m", relative, e);
                    continue;
                }
            };

            let package = buck2::package_from_targets_path(&relative);

            // Store load() statements as import edges
            for load in &parsed.loads {
                let dep_label = buck2::resolve_label(&load.module, &package);
                let symbols_str = load.symbols.join(", ");
                conn.execute(
                    "INSERT OR IGNORE INTO imports (file_path, raw, module, kind) VALUES (?, ?, ?, ?)",
                    params![relative, format!("load({}, {})", load.module, symbols_str), dep_label, "load"],
                ).map_err(|e| e.to_string())?;
            }

            for target in &parsed.targets {
                let srcs_json = serde_json::to_string(&target.srcs).unwrap_or_default();
                let deps_json = serde_json::to_string(&target.deps).unwrap_or_default();
                let exported_json = serde_json::to_string(&target.exported_deps).unwrap_or_default();
                let vis_json = serde_json::to_string(&target.visibility).unwrap_or_default();

                conn.execute(
                    "INSERT OR REPLACE INTO targets (path, name, rule, srcs, deps, exported_deps, visibility) VALUES (?, ?, ?, ?, ?, ?, ?)",
                    params![relative, target.name, target.rule, srcs_json, deps_json, exported_json, vis_json],
                ).map_err(|e| e.to_string())?;

                let label = format!("//{}:{}", package, target.name);
                for dep in &target.deps {
                    let dep_label = buck2::resolve_label(dep, &package);
                    conn.execute(
                        "INSERT OR IGNORE INTO target_edges (source_label, dep_label, kind) VALUES (?, ?, 'dep')",
                        params![label, dep_label],
                    ).map_err(|e| e.to_string())?;
                }
                for dep in &target.exported_deps {
                    let dep_label = buck2::resolve_label(dep, &package);
                    conn.execute(
                        "INSERT OR IGNORE INTO target_edges (source_label, dep_label, kind) VALUES (?, ?, 'exported_dep')",
                        params![label, dep_label],
                    ).map_err(|e| e.to_string())?;
                }
                count += 1;
            }
        }

        conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;
        Ok(())
    })?;

    Ok(count)
}

fn compute_complexity(db: &Database, root: &str, _root_prefix: &str) -> Result<usize, String> {
    use crate::infrastructure::complexity;

    let results = complexity::analyze_directory(root);
    let mut count = 0;

    db.with_conn(|conn| -> Result<(), String> {
        conn.execute_batch("BEGIN").map_err(|e| e.to_string())?;
        for fc in &results {
            let updated = conn.execute(
                "UPDATE files SET complexity = ?, lines = ? WHERE path = ?",
                params![fc.complexity, fc.lines, fc.path],
            ).map_err(|e| e.to_string())?;
            if updated > 0 {
                count += 1;
            }
        }
        conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;
        Ok(())
    })?;
    Ok(count)
}

/// Parse imports using the best parser for each language.
/// C/C++/Python/Java use the file-based parser; others use source-based parsers.
pub fn parse_imports_for_file(file_path: &str) -> Result<Vec<crate::infrastructure::imports::ImportEntry>, String> {
    parse_imports(file_path)
}

fn parse_imports(file_path: &str) -> Result<Vec<crate::infrastructure::imports::ImportEntry>, String> {
    use crate::infrastructure::imports;

    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        // Languages with source-based parsers
        "go" => {
            let source = std::fs::read_to_string(file_path).map_err(|e| e.to_string())?;
            Ok(imports::parse_go_source(&source))
        }
        "rs" => {
            let source = std::fs::read_to_string(file_path).map_err(|e| e.to_string())?;
            Ok(imports::parse_rust_source(&source))
        }
        "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" => {
            let source = std::fs::read_to_string(file_path).map_err(|e| e.to_string())?;
            Ok(imports::parse_js_ts_source(&source))
        }
        "groovy" => {
            let source = std::fs::read_to_string(file_path).map_err(|e| e.to_string())?;
            Ok(imports::parse_groovy_source(&source))
        }
        // CMake files
        _ if file_path.ends_with("CMakeLists.txt") || ext == "cmake" => {
            let source = std::fs::read_to_string(file_path).map_err(|e| e.to_string())?;
            Ok(imports::parse_cmake_source(&source))
        }
        // Gradle files
        _ if file_path.ends_with("build.gradle") => {
            let source = std::fs::read_to_string(file_path).map_err(|e| e.to_string())?;
            Ok(imports::parse_gradle_groovy_source(&source))
        }
        "kts" => {
            let source = std::fs::read_to_string(file_path).map_err(|e| e.to_string())?;
            Ok(imports::parse_gradle_kts_source(&source))
        }
        // Buck2/Starlark/BXL
        "bzl" => {
            let source = std::fs::read_to_string(file_path).map_err(|e| e.to_string())?;
            Ok(imports::parse_starlark_source(&source))
        }
        "bxl" => {
            let source = std::fs::read_to_string(file_path).map_err(|e| e.to_string())?;
            Ok(imports::parse_bxl_source(&source))
        }
        "star" => {
            let source = std::fs::read_to_string(file_path).map_err(|e| e.to_string())?;
            Ok(imports::parse_starlark_source(&source))
        }
        // C/C++/Python/Java/Kotlin — handled by the file-based parser
        _ => imports::parse_includes(file_path),
    }
}

fn detect_language(file_path: &str) -> Option<&str> {
    let ext = std::path::Path::new(file_path).extension().and_then(|e| e.to_str())?;
    match ext {
        "c" | "h" => Some("c"),
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Some("cpp"),
        "py" | "pyi" => Some("python"),
        "java" => Some("java"),
        "go" => Some("go"),
        "js" | "jsx" | "mjs" | "cjs" => Some("javascript"),
        "ts" | "tsx" => Some("typescript"),
        "rs" => Some("rust"),
        "kt" | "kts" => Some("kotlin"),
        _ => None,
    }
}

fn build_edges(db: &Database) -> Result<usize, String> {
    db.with_conn(|conn| -> Result<usize, String> {
        let mut file_index: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        let mut stmt = conn.prepare("SELECT path FROM files").map_err(|e| e.to_string())?;
        let paths: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        for path in &paths {
            if let Some(basename) = std::path::Path::new(path).file_name() {
                file_index
                    .entry(basename.to_string_lossy().to_string())
                    .or_default()
                    .push(path.clone());
            }
        }

        let mut import_stmt = conn
            .prepare("SELECT file_path, module FROM imports")
            .map_err(|e| e.to_string())?;
        let import_rows: Vec<(String, String)> = import_stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        // Build a set of all file paths for quick lookup
        let path_set: std::collections::HashSet<String> = paths.iter().cloned().collect();

        conn.execute_batch("BEGIN").map_err(|e| e.to_string())?;
        let mut edge_count = 0;
        for (source, module) in &import_rows {
            let resolved = resolve_module_to_files(module, &file_index, &path_set);
            for target in &resolved {
                if target != source {
                    conn.execute(
                        "INSERT OR IGNORE INTO edges (source, target, kind) VALUES (?, ?, 'import')",
                        params![source, target],
                    )
                    .map_err(|e| e.to_string())?;
                    edge_count += 1;
                }
            }
        }
        conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;
        Ok(edge_count)
    })
}

/// Resolve a module string to actual file paths in the index.
/// Handles multiple conventions:
///   - C/C++:  "foo/bar.h" → basename match "bar.h"
///   - Rust:   "crate.auth.LoginHandler" → try "src/auth.rs", "src/auth/mod.rs", "src/auth/login_handler.rs"
///   - Python: "src.auth.login" → try "src/auth/login.py", "src/auth.py"
///   - Go:     "github.com/pkg/errors" → basename match
///   - JS/TS:  "./utils" → try "utils.ts", "utils.js", "utils/index.ts"
fn resolve_module_to_files(
    module: &str,
    basename_index: &std::collections::HashMap<String, Vec<String>>,
    path_set: &std::collections::HashSet<String>,
) -> Vec<String> {
    let mut results = Vec::new();

    // Strategy 1: direct basename match (works for C/C++ includes)
    let basename = std::path::Path::new(module)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    if let Some(targets) = basename_index.get(&basename) {
        results.extend(targets.iter().cloned());
    }

    // Strategy 2: convert dotted module path to file path
    // "crate.auth.login" or "std.collections.HashMap" → "auth/login"
    let dotted = module.replace("::", ".").replace("::", "/");
    let segments: Vec<&str> = dotted.split('.').collect();
    if segments.len() >= 2 {
        // Skip "crate", "self", "super" prefixes
        let start = match segments[0] {
            "crate" | "self" | "super" | "std" => 1,
            _ => 0,
        };
        if start < segments.len() {
            let module_path = segments[start..].join("/");

            // Try common extensions
            for ext in &["rs", "py", "go", "java", "kt", "ts", "tsx", "js", "jsx"] {
                // Direct: auth/login.rs
                let candidate = format!("{}.{}", module_path, ext);
                if path_set.contains(&candidate) {
                    results.push(candidate);
                }
                // With src/ prefix: src/auth/login.rs
                let candidate = format!("src/{}.{}", module_path, ext);
                if path_set.contains(&candidate) {
                    results.push(candidate);
                }
            }
            // Rust mod.rs convention: auth/mod.rs
            let mod_candidate = format!("{}/mod.rs", module_path);
            if path_set.contains(&mod_candidate) {
                results.push(mod_candidate);
            }
            let mod_candidate = format!("src/{}/mod.rs", module_path);
            if path_set.contains(&mod_candidate) {
                results.push(mod_candidate);
            }
            // Python __init__.py
            let init_candidate = format!("{}/__init__.py", module_path);
            if path_set.contains(&init_candidate) {
                results.push(init_candidate);
            }
            // JS/TS index files
            for idx in &["index.ts", "index.js", "index.tsx"] {
                let candidate = format!("{}/{}", module_path, idx);
                if path_set.contains(&candidate) {
                    results.push(candidate);
                }
            }
        }
    }

    // Strategy 3: relative path imports (JS/TS "./utils" → "utils.ts")
    if module.starts_with("./") || module.starts_with("../") {
        let clean = module.trim_start_matches("./").trim_start_matches("../");
        for ext in &["ts", "tsx", "js", "jsx"] {
            let candidate = format!("{}.{}", clean, ext);
            if path_set.contains(&candidate) {
                results.push(candidate);
            }
        }
    }

    results.sort();
    results.dedup();
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_python() {
        assert_eq!(detect_language("src/main.py"), Some("python"));
        assert_eq!(detect_language("types.pyi"), Some("python"));
    }

    #[test]
    fn test_detect_cpp() {
        assert_eq!(detect_language("src/main.cpp"), Some("cpp"));
        assert_eq!(detect_language("include/header.hpp"), Some("cpp"));
    }

    #[test]
    fn test_detect_c() {
        assert_eq!(detect_language("src/main.c"), Some("c"));
        assert_eq!(detect_language("include/header.h"), Some("c"));
    }

    #[test]
    fn test_detect_java() {
        assert_eq!(detect_language("src/Main.java"), Some("java"));
    }

    #[test]
    fn test_detect_kotlin() {
        assert_eq!(detect_language("src/App.kt"), Some("kotlin"));
        assert_eq!(detect_language("build.gradle.kts"), Some("kotlin"));
    }

    #[test]
    fn test_detect_go() {
        assert_eq!(detect_language("cmd/server.go"), Some("go"));
    }

    #[test]
    fn test_detect_rust() {
        assert_eq!(detect_language("src/lib.rs"), Some("rust"));
    }

    #[test]
    fn test_detect_typescript() {
        assert_eq!(detect_language("src/app.ts"), Some("typescript"));
        assert_eq!(detect_language("src/Button.tsx"), Some("typescript"));
    }

    #[test]
    fn test_detect_javascript() {
        assert_eq!(detect_language("src/index.js"), Some("javascript"));
        assert_eq!(detect_language("src/App.jsx"), Some("javascript"));
    }

    #[test]
    fn test_detect_unknown() {
        assert_eq!(detect_language("README.md"), None);
        assert_eq!(detect_language("Makefile"), None);
        assert_eq!(detect_language("Dockerfile"), None);
    }
}
