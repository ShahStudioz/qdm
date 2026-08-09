use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "downloads")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub filename: String,
    pub url: String,
    pub save_path: String,
    pub size_downloaded: String,
    pub size_total: String,
    pub state: String,
    pub file_type: String,
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
