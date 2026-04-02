use std::fs;

/// Hash file content using blake3. Returns hex string.
pub fn hash_file(file_path: &str) -> Result<String, String> {
    let content = fs::read(file_path).map_err(|e| format!("Cannot read {}: {}", file_path, e))?;
    let hash = blake3::hash(&content);
    Ok(hash.to_hex().to_string())
}
