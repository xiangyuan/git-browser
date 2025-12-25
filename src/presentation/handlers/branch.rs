use axum::{
    extract::{State, Path},
    response::Json,
};
use std::sync::Arc;
use serde::Serialize;
use crate::presentation::routes::AppContext;
use crate::shared::result::Result;

#[derive(Serialize)]
pub struct BranchDto {
    pub name: String,
    pub target_oid: String,
    pub is_head: bool,
}

/// API: 列出仓库的分支
pub async fn api_list_branches(
    State(ctx): State<Arc<AppContext>>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<BranchDto>>> {
    let repo = ctx.repository_store
        .find_by_id(id)
        .await?
        .ok_or_else(|| crate::shared::error::GitxError::RepositoryNotFound(id.to_string()))?;
    
    let repo_path = std::path::PathBuf::from(&repo.path);
    let branches = ctx.git_client.list_branches(&repo_path).await?;
    
    let dtos: Vec<BranchDto> = branches
        .into_iter()
        .map(|b| BranchDto {
            name: b.name,
            target_oid: b.target_oid,
            is_head: b.is_head,
        })
        .collect();
    
    Ok(Json(dtos))
}
