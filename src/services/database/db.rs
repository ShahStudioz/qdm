use crate::core::utils::paths;
use crate::models::download::{DownloadItem, DownloadState, FileType};
use crate::models::entities::{download, setting};
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, DbErr, EntityTrait,
    QueryOrder, Set,
};

/// Connects to SQLite database at `~/qdm/qdm.db` and auto-creates required tables.
pub async fn init_db() -> Result<DatabaseConnection, DbErr> {
    let sqlite_url = paths::get_sqlite_url();
    let db = Database::connect(&sqlite_url).await?;

    let builder = db.get_database_backend();
    let schema = sea_orm::Schema::new(builder);

    // Auto-create 'downloads' table
    let mut stmt_download = schema.create_table_from_entity(download::Entity);
    stmt_download.if_not_exists();
    let sql_download = builder.build(&stmt_download).to_string();
    db.execute_unprepared(&sql_download).await?;

    // Auto-create 'settings' table
    let mut stmt_setting = schema.create_table_from_entity(setting::Entity);
    stmt_setting.if_not_exists();
    let sql_setting = builder.build(&stmt_setting).to_string();
    db.execute_unprepared(&sql_setting).await?;

    Ok(db)
}

/// Loads all download records from the database ordered by ID descending.
pub async fn load_all_downloads(db: &DatabaseConnection) -> Result<Vec<DownloadItem>, DbErr> {
    let models = download::Entity::find()
        .order_by_desc(download::Column::Id)
        .all(db)
        .await?;

    let items = models
        .into_iter()
        .map(|m| {
            let file_type = FileType::from_filename(&m.filename);
            DownloadItem {
                id: m.id as usize,
                filename: m.filename,
                url: m.url,
                size_downloaded: m.size_downloaded,
                size_total: m.size_total,
                state: match m.state.as_str() {
                    "Completed" => DownloadState::Completed,
                    "Paused" => DownloadState::Paused { progress: 0.0 },
                    err if err.starts_with("Failed:") => DownloadState::Failed {
                        progress: 0.0,
                        error: err.trim_start_matches("Failed:").to_string(),
                    },
                    _ => DownloadState::Downloading {
                        progress: 0.0,
                        speed: "0 B/s".to_string(),
                        eta: "Queued".to_string(),
                    },
                },
                file_type,
            }
        })
        .collect();

    Ok(items)
}

/// Inserts a new download item into the SQLite database and returns the item with assigned DB primary key ID.
pub async fn insert_download(
    db: &DatabaseConnection,
    item: DownloadItem,
    save_path: String,
) -> Result<DownloadItem, DbErr> {
    let active_model = download::ActiveModel {
        filename: Set(item.filename.clone()),
        url: Set(item.url.clone()),
        save_path: Set(save_path),
        size_downloaded: Set(item.size_downloaded.clone()),
        size_total: Set(item.size_total.clone()),
        state: Set("Downloading".to_string()),
        file_type: Set(format!("{:?}", item.file_type)),
        created_at: Set(now_timestamp_str()),
        ..Default::default()
    };

    let res = active_model.insert(db).await?;
    let mut inserted_item = item;
    inserted_item.id = res.id as usize;

    Ok(inserted_item)
}

fn now_timestamp_str() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}
