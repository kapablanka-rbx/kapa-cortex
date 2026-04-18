use crate::domain::entities::{StackPlan, StackPr};
use crate::infrastructure::git;
use std::path::Path;
use std::time::SystemTime;

const PLAN_PATH: &str = ".cortex-cache/stack-plan.json";

fn slugify(title: &str) -> String {
    let result: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<&str>>()
        .join("-");
    result
}

fn generate_branch_name(order: i64, title: &str) -> String {
    let slug = slugify(title);
    let result = format!("stack/{}-{}", order, slug);
    result
}

/// Run analyze and write a stack plan JSON to disk.
pub fn create_plan(base: &str, max_files: usize, max_lines: i64) -> Result<StackPlan, String> {
    let analysis = crate::application::analyze::analyze_branch(base, max_files, max_lines)?;

    let prs: Vec<StackPr> = analysis
        .prs
        .iter()
        .map(|proposed| {
            let branch = generate_branch_name(proposed.order, &proposed.title);
            StackPr::from_proposed(proposed, branch)
        })
        .collect();

    let result = StackPlan {
        version: 1,
        source_branch: analysis.branch,
        base: analysis.base,
        created_at: format_timestamp(),
        prs,
    };

    let plan_json = serde_json::to_string_pretty(&result)
        .map_err(|e| format!("Failed to serialize plan: {}", e))?;

    if let Some(parent) = Path::new(PLAN_PATH).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(PLAN_PATH, &plan_json)
        .map_err(|e| format!("Failed to write plan: {}", e))?;

    Ok(result)
}

/// Read a stack plan from disk and execute it: create branches, commit, push, open PRs.
pub fn apply_plan(plan_path: Option<&str>, dry_run: bool) -> Result<Vec<String>, String> {
    let path = plan_path.unwrap_or(PLAN_PATH);
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read plan: {}", e))?;
    let plan: StackPlan = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse plan: {}", e))?;

    if plan.prs.is_empty() {
        return Err("No PRs in the plan".to_string());
    }

    let mut pr_urls: Vec<String> = Vec::new();

    for stack_pr in &plan.prs {
        let base_branch = determine_base_branch(&plan, stack_pr);

        if dry_run {
            eprintln!("  [dry-run] #{} {} → branch: {} (base: {})",
                stack_pr.order, stack_pr.title, stack_pr.branch, base_branch);
            eprintln!("            files: {}", stack_pr.files.join(", "));
            continue;
        }

        eprintln!("  Creating #{}: {}...", stack_pr.order, stack_pr.title);

        git::create_branch_from(&base_branch, &stack_pr.branch)?;
        git::checkout_files_from(&plan.source_branch, &stack_pr.files)?;
        git::commit(&format!("{}\n\n{}", stack_pr.title, stack_pr.description))?;
        git::push_branch(&stack_pr.branch)?;

        let pr_url = git::open_pr(
            &stack_pr.title,
            &stack_pr.description,
            &base_branch,
        )?;

        eprintln!("  \x1b[32m✓\x1b[0m #{} → {}", stack_pr.order, pr_url);
        pr_urls.push(pr_url);
    }

    if !dry_run {
        git::switch_branch(&plan.source_branch)?;
    }

    Ok(pr_urls)
}

fn format_timestamp() -> String {
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let result = format!("{}", duration.as_secs());
    result
}

fn determine_base_branch<'a>(plan: &'a StackPlan, stack_pr: &StackPr) -> &'a str {
    if stack_pr.depends_on.is_empty() {
        return &plan.base;
    }
    let previous_order = stack_pr.depends_on[0];
    let result = plan
        .prs
        .iter()
        .find(|p| p.order == previous_order)
        .map(|p| p.branch.as_str())
        .unwrap_or(&plan.base);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Add AuthManager"), "add-authmanager");
        assert_eq!(slugify("Update docs"), "update-docs");
        assert_eq!(slugify("Fix  multiple--dashes"), "fix-multiple-dashes");
    }

    #[test]
    fn test_generate_branch_name() {
        let result = generate_branch_name(1, "Add AuthManager");
        assert_eq!(result, "stack/1-add-authmanager");
    }

    #[test]
    fn test_determine_base_branch() {
        let plan = StackPlan {
            version: 1,
            source_branch: "feat/my-feature".to_string(),
            base: "master".to_string(),
            created_at: "2026-04-16T00:00:00Z".to_string(),
            prs: vec![
                StackPr {
                    order: 1,
                    title: "First".to_string(),
                    description: String::new(),
                    files: vec![],
                    risk_level: "low".to_string(),
                    merge_strategy: "squash".to_string(),
                    depends_on: vec![],
                    branch: "stack/1-first".to_string(),
                },
                StackPr {
                    order: 2,
                    title: "Second".to_string(),
                    description: String::new(),
                    files: vec![],
                    risk_level: "low".to_string(),
                    merge_strategy: "squash".to_string(),
                    depends_on: vec![1],
                    branch: "stack/2-second".to_string(),
                },
            ],
        };

        assert_eq!(determine_base_branch(&plan, &plan.prs[0]), "master");
        assert_eq!(determine_base_branch(&plan, &plan.prs[1]), "stack/1-first");
    }
}
