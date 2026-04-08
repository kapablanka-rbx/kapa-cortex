use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: Option<String>,
}

pub struct LlmClient {
    base_url: String,
    model: String,
}

impl LlmClient {
    pub fn new(model: &str) -> Self {
        LlmClient {
            base_url: "http://localhost:11434".to_string(),
            model: model.to_string(),
        }
    }

    pub fn available(&self) -> bool {
        reqwest::blocking::get(&format!("{}/api/tags", self.base_url)).is_ok()
    }

    pub fn generate_title(&self, diff: &str, paths: &[String]) -> Result<String, String> {
        let file_list = paths.iter().take(5).cloned().collect::<Vec<_>>().join(", ");
        let prompt = format!(
            "Given this diff and file list, write a one-line PR title.\nFiles: {}\nDiff:\n{}\nTitle:",
            file_list,
            &diff[..diff.len().min(500)]
        );
        let response = self.generate(&prompt)?;
        // Try to parse as JSON in case the model wraps it
        if let Some(val) = parse_llm_json(&response) {
            if let Some(title) = val.get("title").and_then(|t| t.as_str()) {
                return Ok(title.to_string());
            }
        }
        Ok(response.lines().next().unwrap_or(&response).trim().to_string())
    }

    pub fn generate(&self, prompt: &str) -> Result<String, String> {
        let request = OllamaRequest {
            model: self.model.clone(),
            prompt: prompt.to_string(),
            stream: false,
        };

        let client = reqwest::blocking::Client::new();
        let response = client
            .post(&format!("{}/api/generate", self.base_url))
            .json(&request)
            .send()
            .map_err(|e| format!("LLM request failed: {}", e))?;

        let body: OllamaResponse = response
            .json()
            .map_err(|e| format!("LLM response parse failed: {}", e))?;

        body.response.ok_or_else(|| "Empty LLM response".to_string())
    }
}

/// Parse JSON from LLM response, handling code fences and preamble.
pub fn parse_llm_json(text: &str) -> Option<serde_json::Value> {
    if text.is_empty() {
        return None;
    }
    // Try direct parse
    if let Ok(val) = serde_json::from_str(text) {
        return Some(val);
    }
    // Try extracting from code fence
    if let Some(start) = text.find("```json") {
        let after = &text[start + 7..];
        if let Some(end) = after.find("```") {
            let json_str = after[..end].trim();
            if let Ok(val) = serde_json::from_str(json_str) {
                return Some(val);
            }
        }
    }
    // Try finding first { ... }
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            let json_str = &text[start..=end];
            if let Ok(val) = serde_json::from_str(json_str) {
                return Some(val);
            }
        }
    }
    None
}

/// Generate a PR description without LLM — rule-based fallback.
pub fn rule_based_description(files: &[String]) -> String {
    if files.is_empty() {
        return "Empty change set".to_string();
    }
    let file_list: Vec<&str> = files.iter().map(|f| f.as_str()).take(5).collect();
    format!(
        "Changes to {}{}",
        file_list.join(", "),
        if files.len() > 5 { format!(" and {} more", files.len() - 5) } else { String::new() }
    )
}

/// Generate a title from diff content, file paths, and symbols.
pub fn rule_based_title(diff: &str, paths: &[String], symbols: &[String]) -> String {
    if paths.is_empty() {
        return "Empty change".to_string();
    }
    if let Some(sym) = symbols.first() {
        return format!("Add {}", sym);
    }
    // Check diff for new class/function definitions
    for line in diff.lines() {
        if line.starts_with("+class ") || line.starts_with("+struct ") {
            let name = line.trim_start_matches('+')
                .split_whitespace().nth(1)
                .and_then(|w| w.split(&[':', '(', '{'][..]).next())
                .unwrap_or("");
            if !name.is_empty() {
                return format!("Add {}", name);
            }
        }
    }
    let module = std::path::Path::new(&paths[0]).components().next()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .unwrap_or_else(|| "root".to_string());
    format!("Update {}", module)
}

/// Generate a summary line.
pub fn rule_based_summary(paths: &[String], depends_on: &[i64]) -> String {
    let mut parts = vec![format!("- {} file(s) changed", paths.len())];
    if !depends_on.is_empty() {
        let deps: Vec<String> = depends_on.iter().map(|d| format!("#{}", d)).collect();
        parts.push(format!("- Depends on {}", deps.join(", ")));
    }
    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_clean_json() {
        let val = parse_llm_json("{\"matched\": [\"a.py\"]}").unwrap();
        assert_eq!(val["matched"][0], "a.py");
    }

    #[test]
    fn test_parse_code_fence() {
        let val = parse_llm_json("```json\n{\"foo\": 1}\n```").unwrap();
        assert_eq!(val["foo"], 1);
    }

    #[test]
    fn test_parse_preamble() {
        let val = parse_llm_json("Here is the result:\n{\"bar\": 2}").unwrap();
        assert_eq!(val["bar"], 2);
    }

    #[test]
    fn test_parse_empty() {
        assert!(parse_llm_json("").is_none());
    }

    #[test]
    fn test_rule_based_description_basic() {
        let files = vec!["a.rs".into(), "b.rs".into()];
        let desc = rule_based_description(&files);
        assert!(desc.contains("a.rs"));
    }

    #[test]
    fn test_rule_based_description_many() {
        let files: Vec<String> = (0..10).map(|i| format!("f{}.rs", i)).collect();
        let desc = rule_based_description(&files);
        assert!(desc.contains("5 more"));
    }

    #[test]
    fn test_rule_based_description_empty() {
        assert_eq!(rule_based_description(&[]), "Empty change set");
    }

    #[test]
    fn test_rule_based_title() {
        let title = rule_based_title("", &["src/foo.py".into()], &[]);
        assert!(title.contains("src"));
    }

    #[test]
    fn test_rule_based_title_with_symbol() {
        let title = rule_based_title("", &["src/auth.py".into()], &["AuthManager".into()]);
        assert!(title.contains("AuthManager"));
    }

    #[test]
    fn test_summary_with_deps() {
        let summary = rule_based_summary(&["a.py".into(), "b.py".into()], &[1, 2]);
        assert!(summary.contains("2 file(s)"));
        assert!(summary.contains("Depends on"));
    }

    #[test]
    fn test_summary_no_deps() {
        let summary = rule_based_summary(&["a.py".into()], &[]);
        assert!(!summary.contains("Depends on"));
    }
}
