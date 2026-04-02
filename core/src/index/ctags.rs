use serde::Deserialize;
use std::process::Command;

#[derive(Debug)]
pub struct CtagsSymbol {
    pub name: String,
    pub kind: String,
    pub line: i64,
    pub scope: String,
}

#[derive(Deserialize)]
struct CtagsEntry {
    name: Option<String>,
    kind: Option<String>,
    line: Option<i64>,
    scope: Option<String>,
}

/// Run ctags on a single file, return symbols.
pub fn parse_file(file_path: &str) -> Result<Vec<CtagsSymbol>, String> {
    let output = Command::new("ctags")
        .args([
            "--output-format=json",
            "--fields=+neKS",
            "--kinds-all=*",
            "-f", "-",
            file_path,
        ])
        .output()
        .map_err(|e| format!("ctags failed on {}: {}", file_path, e))?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut symbols = Vec::new();

    for line in stdout.lines() {
        let entry: CtagsEntry = match serde_json::from_str(line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        let name = match entry.name {
            Some(n) if !n.is_empty() => n,
            _ => continue,
        };

        symbols.push(CtagsSymbol {
            name,
            kind: entry.kind.unwrap_or_default(),
            line: entry.line.unwrap_or(0),
            scope: entry.scope.unwrap_or_default(),
        });
    }

    Ok(symbols)
}
