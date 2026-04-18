use std::path::Path;

/// Match files against extraction rules.
pub fn match_files_glob(path: &str, pattern: &str) -> bool {
    if pattern.starts_with("*.") {
        let ext = &pattern[1..];
        path.ends_with(ext)
    } else {
        path.contains(pattern)
    }
}

pub fn match_files_ext(path: &str, ext: &str) -> bool {
    let file_ext = Path::new(path).extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    file_ext == ext
}

pub fn match_files_prefix(path: &str, prefix: &str) -> bool {
    path.starts_with(prefix)
}

pub fn match_files_keyword(diff_text: &str, keyword: &str) -> bool {
    diff_text.contains(keyword)
}

fn strip_visibility(code: &str) -> &str {
    // pub(crate), pub(super), pub
    if code.starts_with("pub(") {
        if let Some(end) = code.find(')') {
            return code[end + 1..].trim_start();
        }
    }
    for prefix in &["pub ", "public ", "export ", "protected ", "private ", "internal "] {
        if code.starts_with(prefix) {
            return &code[prefix.len()..];
        }
    }
    code
}

fn extract_type_name(code: &str) -> Option<String> {
    for keyword in &["struct ", "class ", "enum ", "trait ", "interface ", "type "] {
        if code.starts_with(keyword) {
            let rest = &code[keyword.len()..];
            let name = rest.split(&[' ', '<', '(', '{', ':', ';'][..]).next()?.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn extract_function_name(code: &str) -> Option<String> {
    for keyword in &["fn ", "def ", "func ", "function "] {
        if code.starts_with(keyword) {
            let rest = &code[keyword.len()..];
            let name = rest.split(&['(', '<', ' ', ':'][..]).next()?.trim();
            if !name.is_empty() && name != "main" {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn common_directory(paths: &[String]) -> String {
    if paths.is_empty() {
        return "root".to_string();
    }
    let first_parts: Vec<&str> = paths[0].split('/').collect();
    let mut depth = first_parts.len().saturating_sub(1); // exclude filename

    for path in &paths[1..] {
        let parts: Vec<&str> = path.split('/').collect();
        let max = depth.min(parts.len().saturating_sub(1));
        depth = 0;
        for i in 0..max {
            if first_parts[i] == parts[i] {
                depth = i + 1;
            } else {
                break;
            }
        }
    }

    if depth == 0 {
        first_parts[0].to_string()
    } else {
        first_parts[..depth].join("/")
    }
}

/// Generate a PR title from changed files.
pub fn generate_title(paths: &[String], statuses: &[String], diffs: &[String]) -> String {
    if paths.is_empty() {
        return "Empty PR".to_string();
    }

    // All docs?
    let all_docs = paths.iter().all(|p| {
        let ext = Path::new(p).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        ["md", "txt", "rst", "adoc"].contains(&ext.as_str())
    });
    if all_docs {
        return "Update docs".to_string();
    }

    // All deleted?
    if statuses.iter().all(|s| s == "D") {
        return format!("Remove {}", Path::new(&paths[0]).file_name()
            .map(|f| f.to_string_lossy().to_string()).unwrap_or_default());
    }

    // New type or function definition in diff?
    // Supports: class (Python/Java), struct/enum (Rust/C++/Go),
    // def/fn/func/function keywords across languages.
    for diff in diffs {
        for line in diff.lines() {
            if !line.starts_with('+') || line.starts_with("+++") {
                continue;
            }
            let code = &line[1..].trim_start();

            // Strip visibility: pub, pub(crate), public, export, etc.
            let code = strip_visibility(code);

            if let Some(name) = extract_type_name(code) {
                return format!("Add {}", name);
            }
            if let Some(name) = extract_function_name(code) {
                return format!("Add {}", name);
            }
        }
    }

    // Fallback: deepest common directory
    let module = common_directory(paths);
    format!("Update {}", module)
}

/// Find test file pairs (test_foo.py ↔ foo.py).
pub fn find_test_pairs(paths: &[String]) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for test_path in paths {
        let filename = Path::new(test_path).file_stem()
            .map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
        let ext = Path::new(test_path).extension()
            .map(|e| e.to_string_lossy().to_string()).unwrap_or_default();

        let impl_name = if filename.starts_with("test_") {
            Some(filename.strip_prefix("test_").unwrap().to_string())
        } else if filename.ends_with("_test") {
            Some(filename.strip_suffix("_test").unwrap().to_string())
        } else if filename.ends_with("Test") {
            Some(filename.strip_suffix("Test").unwrap().to_string())
        } else if filename.contains(".test.") || filename.contains(".spec.") {
            let base = filename.replace(".test", "").replace(".spec", "");
            Some(base)
        } else {
            None
        };

        if let Some(impl_stem) = impl_name {
            let dir = Path::new(test_path).parent()
                .map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
            let impl_path = if dir.is_empty() {
                format!("{}.{}", impl_stem, ext)
            } else {
                format!("{}/{}.{}", dir, impl_stem, ext)
            };
            if paths.contains(&impl_path) {
                pairs.push((test_path.clone(), impl_path));
            }
        }
    }
    pairs
}

/// Parse a user prompt into extraction rules.
pub fn parse_prompt(prompt: &str) -> Vec<(String, String)> {
    let mut rules = Vec::new();
    let words: Vec<&str> = prompt.split_whitespace().collect();

    for word in &words {
        if word.starts_with("*.") {
            rules.push(("glob".to_string(), word.to_string()));
        } else if word.ends_with('/') {
            rules.push(("path_prefix".to_string(), word.to_string()));
        } else if *word == "CMakeLists.txt" {
            rules.push(("glob".to_string(), word.to_string()));
        }
    }

    // Keyword detection
    for keyword in &["gradle", "cmake", "docker", "ci", "test", "auth", "config"] {
        if prompt.to_lowercase().contains(keyword) {
            rules.push(("keyword".to_string(), keyword.to_string()));
        }
    }

    rules
}

/// Compute risk score for a set of files.
pub fn compute_risk(code_lines: i64, file_count: usize, dep_count: usize) -> f64 {
    let line_factor = (code_lines as f64 / 500.0).min(1.0);
    let file_factor = (file_count as f64 / 10.0).min(1.0);
    let dep_factor = (dep_count as f64 / 5.0).min(1.0);
    (line_factor * 0.4 + file_factor * 0.3 + dep_factor * 0.3).min(1.0)
}

/// Assign merge strategies to PRs.
pub fn assign_merge_strategy(is_docs: bool, is_depended_upon: bool, risk: f64) -> String {
    if is_docs {
        "rebase".to_string()
    } else if is_depended_upon || risk > 0.5 {
        "merge".to_string()
    } else {
        "squash".to_string()
    }
}

/// Reconstruct old/new sides from unified diff.
pub fn reconstruct_diff_sides(diff: &str) -> (String, String) {
    let mut old_lines = Vec::new();
    let mut new_lines = Vec::new();

    for line in diff.lines() {
        if line.starts_with("@@") || line.starts_with("---") || line.starts_with("+++") {
            continue;
        }
        if line.starts_with('-') {
            old_lines.push(&line[1..]);
        } else if line.starts_with('+') {
            new_lines.push(&line[1..]);
        } else if line.starts_with(' ') {
            old_lines.push(&line[1..]);
            new_lines.push(&line[1..]);
        }
    }

    (old_lines.join("\n"), new_lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── File matching ──

    #[test]
    fn test_glob_match() {
        assert!(match_files_glob("build.gradle", "*.gradle"));
        assert!(!match_files_glob("src/main.py", "*.gradle"));
    }

    #[test]
    fn test_ext_match() {
        assert!(match_files_ext("a.py", ".py"));
        assert!(!match_files_ext("a.py", ".rs"));
    }

    #[test]
    fn test_prefix_match() {
        assert!(match_files_prefix("src/core/a.py", "src/core/"));
        assert!(!match_files_prefix("src/ui/b.py", "src/core/"));
    }

    #[test]
    fn test_keyword_match() {
        assert!(match_files_keyword("+init-script", "init-script"));
        assert!(!match_files_keyword("nothing here", "init-script"));
    }

    // ── PR naming ──

    #[test]
    fn test_title_docs() {
        let title = generate_title(
            &["README.md".into(), "CHANGELOG.md".into()],
            &["M".into(), "M".into()],
            &[],
        );
        assert!(title.to_lowercase().contains("docs"));
    }

    #[test]
    fn test_title_deleted() {
        let title = generate_title(&["old/module.py".into()], &["D".into()], &[]);
        assert!(title.contains("Remove"));
    }

    #[test]
    fn test_title_new_class() {
        let diff = "+class AuthManager:\n+    pass".to_string();
        let title = generate_title(&["src/auth.py".into()], &["A".into()], &[diff]);
        assert!(title.contains("AuthManager"));
    }

    #[test]
    fn test_title_new_function() {
        let diff = "+def validate_token(token):\n+    pass".to_string();
        let title = generate_title(&["src/utils.py".into()], &["M".into()], &[diff]);
        assert!(title.contains("validate_token"));
    }

    #[test]
    fn test_title_rust_struct() {
        let diff = "+pub struct RateLimiter {\n+    max_requests: usize,\n+}".to_string();
        let title = generate_title(&["src/api/rate_limiter.rs".into()], &["A".into()], &[diff]);
        assert!(title.contains("RateLimiter"), "got: {}", title);
    }

    #[test]
    fn test_title_rust_fn() {
        let diff = "+pub fn validate_token(token: &str) -> bool {\n+    true\n+}".to_string();
        let title = generate_title(&["src/auth.rs".into()], &["M".into()], &[diff]);
        assert!(title.contains("validate_token"), "got: {}", title);
    }

    #[test]
    fn test_title_rust_enum() {
        let diff = "+pub enum AuthType {\n+    Session,\n+    Bearer,\n+}".to_string();
        let title = generate_title(&["src/auth.rs".into()], &["M".into()], &[diff]);
        assert!(title.contains("AuthType"), "got: {}", title);
    }

    #[test]
    fn test_title_cpp_class() {
        let diff = "+class btConstraintSolvingStep {\n+public:\n+};".to_string();
        let title = generate_title(&["src/dynamics.cpp".into()], &["A".into()], &[diff]);
        assert!(title.contains("btConstraintSolvingStep"), "got: {}", title);
    }

    #[test]
    fn test_title_go_func() {
        let diff = "+func HandleRequest(w http.ResponseWriter, r *http.Request) {\n+}".to_string();
        let title = generate_title(&["handler.go".into()], &["M".into()], &[diff]);
        assert!(title.contains("HandleRequest"), "got: {}", title);
    }

    #[test]
    fn test_title_ts_function() {
        let diff = "+export function validateEmail(email: string): boolean {\n+}".to_string();
        let title = generate_title(&["src/utils.ts".into()], &["M".into()], &[diff]);
        assert!(title.contains("validateEmail"), "got: {}", title);
    }

    #[test]
    fn test_title_fallback() {
        let title = generate_title(&["src/config.py".into()], &["M".into()], &[]);
        assert!(title.to_lowercase().contains("src"));
    }

    #[test]
    fn test_title_empty() {
        assert_eq!(generate_title(&[], &[], &[]), "Empty PR");
    }

    #[test]
    fn test_title_common_directory() {
        let title = generate_title(
            &["src/auth/login.rs".into(), "src/auth/session.rs".into()],
            &["M".into(), "M".into()],
            &[],
        );
        assert!(title.contains("src/auth"), "got: {}", title);
    }

    // ── Test pair finder ──

    #[test]
    fn test_python_prefix() {
        let paths = vec!["src/test_models.py".into(), "src/models.py".into()];
        let pairs = find_test_pairs(&paths);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].1, "src/models.py");
    }

    #[test]
    fn test_python_suffix() {
        let paths = vec!["src/models_test.py".into(), "src/models.py".into()];
        let pairs = find_test_pairs(&paths);
        assert_eq!(pairs.len(), 1);
    }

    #[test]
    fn test_go_test() {
        let paths = vec!["pkg/handler_test.go".into(), "pkg/handler.go".into()];
        let pairs = find_test_pairs(&paths);
        assert_eq!(pairs.len(), 1);
    }

    #[test]
    fn test_java_test() {
        let paths = vec!["src/FooTest.java".into(), "src/Foo.java".into()];
        let pairs = find_test_pairs(&paths);
        assert_eq!(pairs.len(), 1);
    }

    #[test]
    fn test_cpp_test() {
        let paths = vec!["src/utils_test.cpp".into(), "src/utils.cpp".into()];
        let pairs = find_test_pairs(&paths);
        assert_eq!(pairs.len(), 1);
    }

    #[test]
    fn test_no_pair_when_impl_missing() {
        let paths = vec!["src/test_models.py".into()];
        let pairs = find_test_pairs(&paths);
        assert!(pairs.is_empty());
    }

    // ── Prompt parser ──

    #[test]
    fn test_prompt_gradle() {
        let rules = parse_prompt("gradle files");
        assert!(rules.iter().any(|(_, p)| p.contains("gradle")));
    }

    #[test]
    fn test_prompt_path_prefix() {
        let rules = parse_prompt("src/core/ changes");
        assert!(rules.iter().any(|(k, p)| k == "path_prefix" && p == "src/core/"));
    }

    #[test]
    fn test_prompt_glob() {
        let rules = parse_prompt("the *.bxl files");
        assert!(rules.iter().any(|(k, p)| k == "glob" && p == "*.bxl"));
    }

    #[test]
    fn test_prompt_cmake() {
        let rules = parse_prompt("all CMakeLists.txt changes");
        assert!(rules.iter().any(|(_, p)| p.contains("CMakeLists.txt")));
    }

    // ── Risk scoring ──

    #[test]
    fn test_low_risk() {
        assert!(compute_risk(15, 1, 0) < 0.3);
    }

    #[test]
    fn test_high_risk() {
        assert!(compute_risk(450, 8, 5) > 0.3);
    }

    // ── Merge strategy ──

    #[test]
    fn test_docs_gets_rebase() {
        assert_eq!(assign_merge_strategy(true, false, 0.1), "rebase");
    }

    #[test]
    fn test_depended_upon_gets_merge() {
        assert_eq!(assign_merge_strategy(false, true, 0.1), "merge");
    }

    #[test]
    fn test_standalone_gets_squash() {
        assert_eq!(assign_merge_strategy(false, false, 0.1), "squash");
    }

    #[test]
    fn test_high_risk_gets_merge() {
        assert_eq!(assign_merge_strategy(false, false, 0.7), "merge");
    }

    // ── Diff reconstruction ──

    #[test]
    fn test_reconstruct_sides() {
        let diff = "--- a/foo.py\n+++ b/foo.py\n@@ -1,3 +1,3 @@\n context\n-old_line\n+new_line\n more_context\n";
        let (old, new) = reconstruct_diff_sides(diff);
        assert!(old.contains("old_line"));
        assert!(new.contains("new_line"));
        assert!(old.contains("context"));
        assert!(new.contains("context"));
    }

    #[test]
    fn test_reconstruct_empty() {
        let (old, new) = reconstruct_diff_sides("");
        assert_eq!(old, "");
        assert_eq!(new, "");
    }

    #[test]
    fn test_reconstruct_additions_only() {
        let diff = "@@ -0,0 +1,2 @@\n+line_one\n+line_two\n";
        let (old, new) = reconstruct_diff_sides(diff);
        assert_eq!(old, "");
        assert!(new.contains("line_one"));
    }
}
