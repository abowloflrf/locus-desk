use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};

use crate::{
    api::{ApiJson, ApiQuery},
    error::AppResult,
    notes::{self, CreateNoteRequest, ListNotesResponse, Note, NoteStatus, UpdateNoteRequest},
    state::AppState,
    workspace::RequestContext,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/notes", get(list).post(create))
        .route("/notes/{uid}", get(get_one).patch(update).delete(delete))
        .route("/tags", get(tags))
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ListNotesQuery {
    status: NoteStatus,
    q: Option<String>,
    tag: Option<String>,
    page: u32,
    page_size: u32,
}

impl Default for ListNotesQuery {
    fn default() -> Self {
        Self {
            status: NoteStatus::Active,
            q: None,
            tag: None,
            page: 1,
            page_size: 30,
        }
    }
}

async fn list(
    State(state): State<AppState>,
    context: RequestContext,
    ApiQuery(query): ApiQuery<ListNotesQuery>,
) -> AppResult<Json<ListNotesResponse>> {
    Ok(Json(
        notes::list(
            state.pool(),
            context.workspace_id,
            query.status,
            query.q.as_deref(),
            query.tag.as_deref(),
            query.page,
            query.page_size,
        )
        .await?,
    ))
}

async fn create(
    State(state): State<AppState>,
    context: RequestContext,
    ApiJson(request): ApiJson<CreateNoteRequest>,
) -> AppResult<(StatusCode, Json<Note>)> {
    let note = notes::create(
        state.pool(),
        context.workspace_id,
        context.user_id,
        request,
        state.clock().now().timestamp_millis(),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(note)))
}

async fn get_one(
    State(state): State<AppState>,
    context: RequestContext,
    Path(uid): Path<String>,
) -> AppResult<Json<Note>> {
    Ok(Json(
        notes::get(state.pool(), context.workspace_id, &uid).await?,
    ))
}

async fn update(
    State(state): State<AppState>,
    context: RequestContext,
    Path(uid): Path<String>,
    ApiJson(request): ApiJson<UpdateNoteRequest>,
) -> AppResult<Json<Note>> {
    Ok(Json(
        notes::update(
            state.pool(),
            context.workspace_id,
            &uid,
            request,
            state.clock().now().timestamp_millis(),
        )
        .await?,
    ))
}

async fn delete(
    State(state): State<AppState>,
    context: RequestContext,
    Path(uid): Path<String>,
) -> AppResult<StatusCode> {
    notes::delete(state.pool(), context.workspace_id, &uid).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
struct ListTagsResponse {
    items: Vec<String>,
}

async fn tags(
    State(state): State<AppState>,
    context: RequestContext,
) -> AppResult<Json<ListTagsResponse>> {
    Ok(Json(ListTagsResponse {
        items: notes::list_tags(state.pool(), context.workspace_id).await?,
    }))
}
