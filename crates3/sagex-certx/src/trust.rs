use std::path::Path;

use error::AppError;
use sagex_keys::PublicKey;

use crate::error;

pub async fn load(dir: &Path) -> Result<Vec<PublicKey>, AppError> {
    let mut entries = tokio::fs::read_dir(dir)
        .await
        .map_err(|e| AppError::ReadDir(dir.to_path_buf(), e))?;
    let mut keys = Vec::new();
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| AppError::ReadDir(dir.to_path_buf(), e))?
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("pub") {
            continue;
        }
        let bytes = tokio::fs::read(&path)
            .await
            .map_err(|e| AppError::ReadKey(path.clone(), e))?;
        let key =
            PublicKey::from_bytes(&bytes).map_err(|e| AppError::BadKey(path.clone(), e))?;
        keys.push(key);
    }
    Ok(keys)
}
