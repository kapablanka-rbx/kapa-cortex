use crate::domain::entities::ChangedFile;
use crate::domain::services;
use crate::infrastructure::git;

pub struct ExtractionResult {
    pub description: String,
    pub matched_files: Vec<String>,
    pub unmatched_files: Vec<String>,
}

/// Create a branch with only the matched files cherry-picked from the current branch.
pub fn create_extraction_branch(
    base: &str,
    branch_name: &str,
    files: &[String],
) -> Result<(), String> {
    git::cherry_pick_files(base, branch_name, files)
}

/// Extract files matching a user description from the current branch.
pub fn extract_files(base: &str, description: &str) -> Result<ExtractionResult, String> {
    let files = git::diff_stat(base)?;
    if files.is_empty() {
        return Ok(ExtractionResult {
            description: description.to_string(),
            matched_files: Vec::new(),
            unmatched_files: Vec::new(),
        });
    }

    // Populate diff text for keyword matching
    let files_with_diffs: Vec<(ChangedFile, String)> = files
        .into_iter()
        .map(|f| {
            let diff = git::diff_text(base, &f.path).unwrap_or_default();
            (f, diff)
        })
        .collect();

    let rules = services::parse_prompt(description);

    let mut matched = Vec::new();
    let mut unmatched = Vec::new();

    for (file, diff) in &files_with_diffs {
        // Reconstruct new side of diff for keyword matching on actual content
        let (_old_side, new_side) = services::reconstruct_diff_sides(diff);
        let search_text = format!("{}\n{}", diff, new_side);

        let is_match = rules.iter().any(|(kind, pattern)| match kind.as_str() {
            "glob" => services::match_files_glob(&file.path, pattern),
            "path_prefix" => services::match_files_prefix(&file.path, pattern),
            "keyword" => services::match_files_keyword(&search_text, pattern),
            "ext" => services::match_files_ext(&file.path, pattern),
            _ => false,
        });

        if is_match {
            matched.push(file.path.clone());
        } else {
            unmatched.push(file.path.clone());
        }
    }

    // Pull in test pairs
    let all_paths: Vec<String> = files_with_diffs.iter().map(|(f, _)| f.path.clone()).collect();
    let test_pairs = services::find_test_pairs(&all_paths);
    for (test_file, impl_file) in &test_pairs {
        if matched.contains(impl_file) && !matched.contains(test_file) {
            matched.push(test_file.clone());
            unmatched.retain(|f| f != test_file);
        }
    }

    Ok(ExtractionResult {
        description: description.to_string(),
        matched_files: matched,
        unmatched_files: unmatched,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_prompt_rules() {
        let rules = services::parse_prompt("*.gradle changes in src/auth/");
        assert!(rules.iter().any(|(k, _)| k == "glob"));
        assert!(rules.iter().any(|(k, _)| k == "path_prefix"));
    }
}
