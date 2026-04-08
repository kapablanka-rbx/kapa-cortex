use crate::domain::entities::{AnalysisResult, ChangedFile, ProposedPr};
use crate::domain::services;
use crate::infrastructure::{complexity, git, llm};
use std::collections::HashMap;

/// Analyze a branch and propose stacked PRs.
pub fn analyze_branch(base: &str, max_files: usize, max_lines: i64) -> Result<AnalysisResult, String> {
    let branch = git::current_branch()?;
    let mut files = git::diff_stat(base)?;

    if files.is_empty() {
        return Ok(AnalysisResult {
            branch,
            base: base.to_string(),
            files: Vec::new(),
            prs: Vec::new(),
        });
    }

    // Populate diff text and complexity for each file
    let complexity_cache = complexity::load_complexity_cache(".");
    for file in &mut files {
        if let Ok(diff) = git::diff_text(base, &file.path) {
            file.diff_text = diff;
        }
        if let Some(ref cache) = complexity_cache {
            if let Some(fc) = cache.get(&file.path) {
                file.complexity = fc.complexity;
            }
        }
    }

    let prs = group_into_prs(&files, max_files, max_lines);

    Ok(AnalysisResult {
        branch,
        base: base.to_string(),
        files,
        prs,
    })
}

/// Public entry point for tests — delegates to the grouping algorithm.
pub fn group_into_prs_pub(files: &[ChangedFile], max_files: usize, max_lines: i64) -> Vec<ProposedPr> {
    group_into_prs(files, max_files, max_lines)
}

fn group_into_prs(files: &[ChangedFile], max_files: usize, max_lines: i64) -> Vec<ProposedPr> {
    // Group by module (top-level directory)
    let mut groups: HashMap<String, Vec<&ChangedFile>> = HashMap::new();
    for file in files {
        let key = file.module_key();
        groups.entry(key).or_default().push(file);
    }

    // Find test pairs to keep together
    let all_paths: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
    let test_pairs = services::find_test_pairs(&all_paths);
    let paired_tests: std::collections::HashSet<String> =
        test_pairs.iter().map(|(test, _)| test.clone()).collect();

    let mut prs = Vec::new();
    let mut order: i64 = 1;

    // Text/docs files go first
    let doc_files: Vec<String> = files
        .iter()
        .filter(|f| f.is_text_or_docs())
        .map(|f| f.path.clone())
        .collect();
    if !doc_files.is_empty() {
        prs.push(ProposedPr {
            title: "Documentation updates".to_string(),
            description: llm::rule_based_description(&doc_files),
            files: doc_files,
            order,
            risk_level: "low".to_string(),
            merge_strategy: services::assign_merge_strategy(true, false, 0.0),
            depends_on: Vec::new(),
        });
        order += 1;
    }

    // Split remaining files by module, respecting max_files and max_lines
    for (module, module_files) in &groups {
        let code_files: Vec<&ChangedFile> = module_files
            .iter()
            .filter(|f| !f.is_text_or_docs())
            .cloned()
            .collect();
        if code_files.is_empty() {
            continue;
        }

        let mut current_batch: Vec<&ChangedFile> = Vec::new();
        let mut current_lines: i64 = 0;

        for file in &code_files {
            // Skip tests that are already paired with their implementation
            if paired_tests.contains(&file.path) {
                continue;
            }

            if (current_batch.len() >= max_files || current_lines + file.code_lines() > max_lines)
                && !current_batch.is_empty()
            {
                let pr = build_pr(&current_batch, module, order, &test_pairs);
                prs.push(pr);
                order += 1;
                current_batch.clear();
                current_lines = 0;
            }
            current_batch.push(file);
            current_lines += file.code_lines();
        }

        if !current_batch.is_empty() {
            let pr = build_pr(&current_batch, module, order, &test_pairs);
            prs.push(pr);
            order += 1;
        }
    }

    prs
}

fn build_pr(
    batch: &[&ChangedFile],
    module: &str,
    order: i64,
    test_pairs: &[(String, String)],
) -> ProposedPr {
    let mut pr_files: Vec<String> = batch.iter().map(|f| f.path.clone()).collect();

    // Pull in paired test files
    for file in batch {
        for (test, impl_file) in test_pairs {
            if impl_file == &file.path {
                pr_files.push(test.clone());
            }
        }
    }

    let paths: Vec<String> = batch.iter().map(|f| f.path.clone()).collect();
    let statuses: Vec<String> = batch.iter().map(|f| f.status.clone()).collect();
    let diffs: Vec<String> = batch.iter().map(|f| f.diff_text.clone()).collect();

    let total_lines: i64 = batch.iter().map(|f| f.code_lines()).sum();
    let dep_count = 0_usize; // TODO: wire from index when available
    let risk = services::compute_risk(total_lines, pr_files.len(), dep_count);
    let risk_level = if risk > 0.6 { "high" } else if risk > 0.3 { "medium" } else { "low" };
    let is_depended_upon = order == 1;

    let title = services::generate_title(&paths, &statuses, &diffs);
    let combined_diff = diffs.join("\n");
    let symbols: Vec<String> = Vec::new(); // TODO: extract from index
    let llm_title = llm::rule_based_title(&combined_diff, &paths, &symbols);
    // Prefer domain services title (diff-aware), fall back to LLM title
    let best_title = if title.starts_with("Update") && !llm_title.starts_with("Update") {
        llm_title
    } else {
        title
    };
    let depends_on: Vec<i64> = if order > 1 { vec![order - 1] } else { Vec::new() };
    let description = llm::rule_based_summary(&pr_files, &depends_on);

    ProposedPr {
        title: if best_title.starts_with("Update") && paths.len() > 1 {
            format!("{} changes", module)
        } else {
            best_title
        },
        description,
        files: pr_files,
        order,
        risk_level: risk_level.to_string(),
        merge_strategy: services::assign_merge_strategy(false, is_depended_upon, risk),
        depends_on,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, added: i64, removed: i64) -> ChangedFile {
        ChangedFile {
            path: path.to_string(), added, removed,
            status: "M".to_string(), diff_text: String::new(),
            complexity: 0, structural_ratio: 1.0,
        }
    }

    #[test]
    fn test_empty_files_no_prs() {
        let prs = group_into_prs(&[], 3, 200);
        assert!(prs.is_empty());
    }

    #[test]
    fn test_docs_go_first() {
        let files = vec![file("src/main.rs", 50, 10), file("README.md", 5, 0)];
        let prs = group_into_prs(&files, 3, 200);
        assert_eq!(prs[0].title, "Documentation updates");
        assert!(prs[0].files.contains(&"README.md".to_string()));
    }

    #[test]
    fn test_split_by_max_files() {
        let files = vec![
            file("src/a.rs", 10, 0),
            file("src/b.rs", 10, 0),
            file("src/c.rs", 10, 0),
            file("src/d.rs", 10, 0),
        ];
        let prs = group_into_prs(&files, 2, 1000);
        assert!(prs.len() >= 2);
    }

    #[test]
    fn test_split_by_max_lines() {
        let files = vec![
            file("src/a.rs", 150, 0),
            file("src/b.rs", 150, 0),
        ];
        let prs = group_into_prs(&files, 10, 200);
        assert!(prs.len() >= 2);
    }

    #[test]
    fn test_compute_risk_low() {
        let risk = services::compute_risk(15, 1, 0);
        assert!(risk < 0.3);
    }

    #[test]
    fn test_compute_risk_high() {
        let risk = services::compute_risk(450, 8, 5);
        assert!(risk > 0.3);
    }

    #[test]
    fn test_docs_pr_gets_rebase_strategy() {
        let files = vec![file("README.md", 5, 0), file("src/main.rs", 50, 10)];
        let prs = group_into_prs(&files, 3, 200);
        let docs_pr = prs.iter().find(|p| p.title == "Documentation updates");
        assert!(docs_pr.is_some());
        assert_eq!(docs_pr.unwrap().merge_strategy, "rebase");
    }

    #[test]
    fn test_rule_based_description() {
        let files = vec!["a.rs".to_string(), "b.rs".to_string()];
        let desc = llm::rule_based_description(&files);
        assert!(desc.contains("a.rs"));
        assert!(desc.contains("b.rs"));
    }

    #[test]
    fn test_rule_based_description_many_files() {
        let files: Vec<String> = (0..10).map(|i| format!("file{}.rs", i)).collect();
        let desc = llm::rule_based_description(&files);
        assert!(desc.contains("5 more"));
    }
}
