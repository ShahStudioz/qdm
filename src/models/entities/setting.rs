use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "settings")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    pub download_folder: String,
    pub simultaneous_downloads: i32,
    pub launch_at_startup: bool,
    pub minimize_to_tray: bool,
    pub show_notifications: bool,
    pub notification_sound: String,
    pub auto_check_updates: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
