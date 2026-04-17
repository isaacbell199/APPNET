//! Backup management for NexaStory
//! Automatic backups are stored in data/backups/

use anyhow::Result;
use chrono::Local;
use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// Backup metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    pub id: String,
    pub filename: String,
    pub created_at: String,
    pub size_bytes: u64,
    pub project_count: usize,
    pub chapter_count: usize,
    pub is_auto: bool,
}

/// Get counts from the database (helper function)
async fn get_database_counts() -> Result<(usize, usize)> {
    // Try to get the database pool from the database module
    // We'll query directly for counts
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:./data/nexastory.db".to_string());
    
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await?;
    
    let project_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM projects")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);
    
    let chapter_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chapters")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);
    
    pool.close().await;
    
    Ok((project_count as usize, chapter_count as usize))
}

/// Get the backups directory path
pub fn get_backups_dir() -> PathBuf {
    // Try environment variable first
    if let Ok(data_dir) = std::env::var("NEXASTORY_DATA_DIR") {
        return PathBuf::from(data_dir).join("backups");
    }

    // Fallback
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("data")
        .join("backups")
}

/// Ensure backups directory exists
pub fn ensure_backups_dir() -> Result<PathBuf> {
    let dir = get_backups_dir();
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Create a full database backup
pub async fn create_backup(db_url: &str, is_auto: bool) -> Result<BackupInfo> {
    let backups_dir = ensure_backups_dir()?;

    // Generate backup filename
    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
    let prefix = if is_auto { "auto" } else { "manual" };
    let filename = format!("{}_backup_{}.db", prefix, timestamp);
    let backup_path = backups_dir.join(&filename);

    // Copy the database file
    // Parse the SQLite URL to get the file path
    let db_path = db_url
        .strip_prefix("sqlite:")
        .map(|s| s.split('?').next().unwrap_or(s))
        .ok_or_else(|| anyhow::anyhow!("Invalid database URL"))?;

    fs::copy(db_path, &backup_path)?;

    // Get file size
    let metadata = fs::metadata(&backup_path)?;
    let size_bytes = metadata.len();

    // Query the database for actual project and chapter counts
    let (project_count, chapter_count) = get_database_counts().await
        .unwrap_or((0, 0));

    let backup_info = BackupInfo {
        id: timestamp.to_string(),
        filename,
        created_at: Local::now().to_rfc3339(),
        size_bytes,
        project_count,
        chapter_count,
        is_auto,
    };

    log::info!(
        "Created {} backup: {} ({} bytes, {} projects, {} chapters)",
        if is_auto { "automatic" } else { "manual" },
        backup_info.filename,
        size_bytes,
        project_count,
        chapter_count
    );

    Ok(backup_info)
}

/// List all available backups
pub fn list_backups() -> Result<Vec<BackupInfo>> {
    let backups_dir = get_backups_dir();

    if !backups_dir.exists() {
        return Ok(vec![]);
    }

    let mut backups = Vec::new();

    for entry in fs::read_dir(&backups_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().map(|e| e == "db").unwrap_or(false) {
            let filename = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let metadata = fs::metadata(&path)?;
            let created_at = metadata.modified()
                .map(|t| {
                    let datetime: chrono::DateTime<Local> = t.into();
                    datetime.to_rfc3339()
                })
                .unwrap_or_else(|_| "Unknown".to_string());

            let is_auto = filename.starts_with("auto_");

            backups.push(BackupInfo {
                id: filename.clone(),
                filename,
                created_at,
                size_bytes: metadata.len(),
                project_count: 0,
                chapter_count: 0,
                is_auto,
            });
        }
    }

    // Sort by creation date, newest first
    backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(backups)
}

/// Restore database from a backup
pub fn restore_backup(db_url: &str, backup_filename: &str) -> Result<()> {
    let backups_dir = get_backups_dir();
    let backup_path = backups_dir.join(backup_filename);

    if !backup_path.exists() {
        return Err(anyhow::anyhow!("Backup file not found: {}", backup_filename));
    }

    // Parse the SQLite URL to get the file path
    let db_path = db_url
        .strip_prefix("sqlite:")
        .map(|s| s.split('?').next().unwrap_or(s))
        .ok_or_else(|| anyhow::anyhow!("Invalid database URL"))?;

    // Create a backup of current database before restore
    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
    let pre_restore_backup = format!("pre_restore_{}.db", timestamp);
    let pre_restore_path = backups_dir.join(&pre_restore_backup);

    if fs::metadata(db_path).is_ok() {
        fs::copy(db_path, &pre_restore_path)?;
        log::info!("Created pre-restore backup: {}", pre_restore_backup);
    }

    // Copy the backup to the database location
    fs::copy(&backup_path, db_path)?;

    log::info!("Restored database from backup: {}", backup_filename);

    Ok(())
}

/// Delete a backup file
pub fn delete_backup(backup_filename: &str) -> Result<()> {
    let backups_dir = get_backups_dir();
    let backup_path = backups_dir.join(backup_filename);

    if backup_path.exists() {
        fs::remove_file(&backup_path)?;
        log::info!("Deleted backup: {}", backup_filename);
    }

    Ok(())
}

/// Clean up old automatic backups, keeping only the most recent N
pub fn cleanup_old_backups(keep_count: usize) -> Result<usize> {
    let backups = list_backups()?;
    let auto_backups: Vec<_> = backups.iter()
        .filter(|b| b.is_auto)
        .collect();

    let mut deleted = 0;

    // Delete old auto backups
    if auto_backups.len() > keep_count {
        for backup in auto_backups.iter().skip(keep_count) {
            if delete_backup(&backup.filename).is_ok() {
                deleted += 1;
            }
        }
    }

    Ok(deleted)
}

/// Get the exports directory path
pub fn get_exports_dir() -> PathBuf {
    // Try environment variable first
    if let Ok(data_dir) = std::env::var("NEXASTORY_DATA_DIR") {
        return PathBuf::from(data_dir).join("exports");
    }

    // Fallback
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("data")
        .join("exports")
}

/// Ensure exports directory exists
pub fn ensure_exports_dir() -> Result<PathBuf> {
    let dir = get_exports_dir();
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Save export to file
pub fn save_export(filename: &str, content: &str) -> Result<String> {
    let exports_dir = ensure_exports_dir()?;
    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
    let final_filename = if filename.is_empty() {
        format!("export_{}.json", timestamp)
    } else {
        format!("{}_{}.json", filename, timestamp)
    };

    let export_path = exports_dir.join(&final_filename);
    fs::write(&export_path, content)?;

    log::info!("Saved export to: {}", export_path.display());

    Ok(export_path.to_string_lossy().to_string())
}
