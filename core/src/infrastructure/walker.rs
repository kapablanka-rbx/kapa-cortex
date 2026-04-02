use std::path::Path;

const SOURCE_EXTENSIONS: &[&str] = &[
    ".py", ".pyi", ".c", ".h", ".cc", ".cpp", ".cxx", ".hpp",
    ".java", ".kt", ".kts", ".go", ".rs",
    ".js", ".jsx", ".ts", ".tsx", ".mjs", ".cjs",
];

const SKIP_DIRS: &[&str] = &[
    ".git", "node_modules", "__pycache__", ".mypy_cache",
    "venv", ".venv", "env", ".env", "dist", "build",
    ".cortex-cache", ".tox", ".pytest_cache", ".cache",
];

pub fn find_source_files(root: &str) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    walk_dir(Path::new(root), &mut files)?;
    Ok(files)
}

fn walk_dir(dir: &Path, files: &mut Vec<String>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("Cannot read {}: {}", dir.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_dir() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !SKIP_DIRS.contains(&name_str.as_ref()) {
                walk_dir(&path, files)?;
            }
        } else if let Some(ext) = path.extension() {
            let ext_with_dot = format!(".{}", ext.to_string_lossy());
            if SOURCE_EXTENSIONS.contains(&ext_with_dot.as_str()) {
                files.push(path.to_string_lossy().to_string());
            }
        }
    }

    Ok(())
}
