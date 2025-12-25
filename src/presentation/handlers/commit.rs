use axum::{
    extract::{State, Path, Query},
    response::Json,
};
use std::sync::Arc;
use serde::Deserialize;
use crate::presentation::routes::AppContext;
use crate::presentation::dto::CommitDto;
use crate::shared::result::Result;

#[derive(Deserialize)]
pub struct ListCommitsQuery {
    pub branch: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// API: 列出仓库的提交
pub async fn api_list_commits(
    State(ctx): State<Arc<AppContext>>,
    Path(id): Path<i64>,
    Query(query): Query<ListCommitsQuery>,
) -> Result<Json<Vec<CommitDto>>> {
    let commits = ctx.commit_store.list_by_repository(
        id,
        query.branch.as_deref(),
        query.limit.unwrap_or(100),
        query.offset.unwrap_or(0),
    ).await?;
    
    let dtos: Vec<CommitDto> = commits.into_iter().map(Into::into).collect();
    
    Ok(Json(dtos))
}

/// API: 获取单个提交详情
pub async fn api_get_commit(
    State(ctx): State<Arc<AppContext>>,
    Path((repo_id, oid)): Path<(i64, String)>,
) -> Result<Json<CommitDto>> {
    let commit = ctx.commit_store
        .find_by_oid(repo_id, &oid)
        .await?
        .ok_or_else(|| crate::shared::error::GitxError::CommitNotFound(oid))?;
    
    Ok(Json(commit.into()))
}
