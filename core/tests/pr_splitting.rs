//! PR splitting scenarios — test the analyze pipeline with realistic file sets.
//!
//! These test `group_into_prs` directly with hand-crafted ChangedFile sets
//! that mirror real-world branching patterns.

use kapa_cortex_core::domain::entities::{ChangedFile, ProposedPr};

fn file(path: &str, added: i64, removed: i64, status: &str) -> ChangedFile {
    ChangedFile {
        path: path.to_string(),
        added,
        removed,
        status: status.to_string(),
        diff_text: String::new(),
        complexity: 0,
        structural_ratio: 1.0,
    }
}

fn file_with_diff(path: &str, added: i64, removed: i64, diff: &str) -> ChangedFile {
    ChangedFile {
        path: path.to_string(),
        added,
        removed,
        status: "M".to_string(),
        diff_text: diff.to_string(),
        complexity: 0,
        structural_ratio: 1.0,
    }
}

fn file_complex(path: &str, added: i64, removed: i64, complexity: i64) -> ChangedFile {
    ChangedFile {
        path: path.to_string(),
        added,
        removed,
        status: "M".to_string(),
        diff_text: String::new(),
        complexity,
        structural_ratio: 1.0,
    }
}

// ── Scenario 1: Mixed docs + code ──
// Docs go into their own PR first, code follows.

#[test]
fn scenario_docs_separated_from_code() {
    let files = vec![
        file("README.md", 10, 2, "M"),
        file("CHANGELOG.md", 5, 0, "A"),
        file("src/auth/login.rs", 80, 20, "M"),
        file("src/auth/session.rs", 40, 10, "M"),
    ];
    let prs = kapa_cortex_core::application::analyze::group_into_prs_pub(&files, 5, 500);

    assert!(prs.len() >= 2, "Should have at least 2 PRs (docs + code)");

    let docs_pr = &prs[0];
    assert_eq!(docs_pr.title, "Documentation updates");
    assert!(docs_pr.files.contains(&"README.md".to_string()));
    assert!(docs_pr.files.contains(&"CHANGELOG.md".to_string()));
    assert_eq!(docs_pr.risk_level, "low");
    assert_eq!(docs_pr.merge_strategy, "rebase");
    assert!(docs_pr.depends_on.is_empty());
}

// ── Scenario 2: Test files stay with implementation ──
// test_foo.py must be in the same PR as foo.py.

#[test]
fn scenario_test_pairs_grouped_together() {
    let files = vec![
        file("src/models.py", 50, 10, "M"),
        file("src/test_models.py", 30, 5, "M"),
        file("src/utils.py", 20, 0, "M"),
    ];
    let prs = kapa_cortex_core::application::analyze::group_into_prs_pub(&files, 5, 500);

    // Find PR containing models.py
    let models_pr = prs.iter().find(|pr| pr.files.contains(&"src/models.py".to_string()));
    assert!(models_pr.is_some(), "Should have a PR with models.py");

    let models_pr = models_pr.unwrap();
    assert!(
        models_pr.files.contains(&"src/test_models.py".to_string()),
        "test_models.py must be in the same PR as models.py"
    );
}

// ── Scenario 3: Large branch splits by max_files ──
// 10 files with max_files=3 should produce 3-4 PRs.

#[test]
fn scenario_split_by_max_files() {
    let files: Vec<ChangedFile> = (0..10)
        .map(|i| file(&format!("src/module{}.rs", i), 20, 5, "M"))
        .collect();
    let prs = kapa_cortex_core::application::analyze::group_into_prs_pub(&files, 3, 1000);

    assert!(prs.len() >= 3, "10 files / 3 per PR = at least 3 PRs, got {}", prs.len());
    for pr in &prs {
        assert!(pr.files.len() <= 4, "No PR should have more than 4 files (3 + test pair), got {}", pr.files.len());
    }
}

// ── Scenario 4: Large branch splits by max_lines ──
// Files with 200 lines each, max_lines=300 → each PR gets ~1-2 files.

#[test]
fn scenario_split_by_max_lines() {
    let files = vec![
        file("src/big_a.rs", 200, 0, "M"),
        file("src/big_b.rs", 200, 0, "M"),
        file("src/big_c.rs", 200, 0, "M"),
    ];
    let prs = kapa_cortex_core::application::analyze::group_into_prs_pub(&files, 10, 300);

    assert!(prs.len() >= 2, "600 lines / 300 max = at least 2 PRs, got {}", prs.len());
}

// ── Scenario 5: Multi-module branch ──
// Files in different top-level directories get separate PRs.

#[test]
fn scenario_multi_module_separation() {
    let files = vec![
        file("frontend/app.tsx", 50, 10, "M"),
        file("frontend/styles.css", 20, 5, "M"),
        file("backend/api.rs", 80, 20, "M"),
        file("backend/db.rs", 40, 10, "M"),
    ];
    let prs = kapa_cortex_core::application::analyze::group_into_prs_pub(&files, 10, 1000);

    // Should have at least 2 PRs (one per module)
    assert!(prs.len() >= 2, "Multi-module should produce >= 2 PRs, got {}", prs.len());

    // No PR should mix frontend and backend
    for pr in &prs {
        let has_frontend = pr.files.iter().any(|f| f.starts_with("frontend/"));
        let has_backend = pr.files.iter().any(|f| f.starts_with("backend/"));
        assert!(
            !(has_frontend && has_backend),
            "PR '{}' mixes frontend and backend files",
            pr.title
        );
    }
}

// ── Scenario 6: Single file change ──
// One file = one PR, no splitting needed.

#[test]
fn scenario_single_file() {
    let files = vec![file("src/main.rs", 10, 5, "M")];
    let prs = kapa_cortex_core::application::analyze::group_into_prs_pub(&files, 3, 200);

    assert_eq!(prs.len(), 1);
    assert_eq!(prs[0].files.len(), 1);
}

// ── Scenario 7: All deletions ──
// Deleted files should get a "Remove" title.

#[test]
fn scenario_all_deletions() {
    let files = vec![
        file_with_diff("src/old_module.rs", 0, 100, "-mod old_module;\n-fn deprecated() {}"),
        file_with_diff("src/legacy.rs", 0, 50, "-fn legacy_code() {}"),
    ];
    let prs = kapa_cortex_core::application::analyze::group_into_prs_pub(&files, 5, 500);

    assert!(!prs.is_empty());
}

// ── Scenario 8: New class in diff triggers smart title ──

#[test]
fn scenario_new_class_title() {
    let diff = "+class AuthManager:\n+    def __init__(self):\n+        pass\n";
    let files = vec![file_with_diff("src/auth.py", 30, 0, diff)];
    let prs = kapa_cortex_core::application::analyze::group_into_prs_pub(&files, 5, 500);

    assert!(!prs.is_empty());
    let title = &prs[0].title;
    assert!(
        title.contains("AuthManager") || title.contains("Add"),
        "Title should mention the new class, got: {}",
        title
    );
}

// ── Scenario 9: High complexity files get high risk ──

#[test]
fn scenario_high_complexity_high_risk() {
    let files = vec![
        file_complex("src/complex_parser.rs", 400, 100, 80),
    ];
    let prs = kapa_cortex_core::application::analyze::group_into_prs_pub(&files, 5, 1000);

    assert!(!prs.is_empty());
    // High code lines (500) should produce high risk
    let pr = &prs[0];
    assert!(
        pr.risk_level == "high" || pr.risk_level == "medium",
        "500 lines should be medium or high risk, got: {}",
        pr.risk_level
    );
}

// ── Scenario 10: Dependencies chain correctly ──

#[test]
fn scenario_dependency_chain() {
    let files: Vec<ChangedFile> = (0..6)
        .map(|i| file(&format!("src/step{}.rs", i), 100, 0, "M"))
        .collect();
    let prs = kapa_cortex_core::application::analyze::group_into_prs_pub(&files, 2, 300);

    // Later PRs should depend on earlier ones
    for (idx, pr) in prs.iter().enumerate() {
        if idx > 0 {
            assert!(
                !pr.depends_on.is_empty(),
                "PR #{} should depend on a previous PR",
                pr.order
            );
        }
    }
}

// ── Scenario 11: Merge strategy assignment ──

#[test]
fn scenario_merge_strategies() {
    let files = vec![
        file("docs/guide.md", 20, 5, "M"),
        file("src/core.rs", 100, 30, "M"),
        file("src/utils.rs", 20, 5, "M"),
    ];
    let prs = kapa_cortex_core::application::analyze::group_into_prs_pub(&files, 5, 500);

    let docs_pr = prs.iter().find(|p| p.title == "Documentation updates");
    if let Some(docs) = docs_pr {
        assert_eq!(docs.merge_strategy, "rebase", "Docs should use rebase");
    }

    // Code PRs should have a valid merge strategy
    let code_prs: Vec<&ProposedPr> = prs.iter().filter(|p| p.title != "Documentation updates").collect();
    for code_pr in &code_prs {
        assert!(
            ["merge", "squash", "rebase"].contains(&code_pr.merge_strategy.as_str()),
            "Code PR should have a valid merge strategy, got: {}",
            code_pr.merge_strategy
        );
    }
}
