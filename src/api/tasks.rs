use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use serde::Deserialize;

use crate::{
    api::{ApiJson, ApiQuery},
    error::AppResult,
    state::AppState,
    tasks::{
        self, CreateTaskRequest, ListTasksResponse, Task, TaskScope, TaskStatus, UpdateTaskRequest,
    },
    workspace::RequestContext,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tasks", get(list).post(create))
        .route("/tasks/{uid}", get(get_one).patch(update).delete(delete))
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ListTasksQuery {
    scope: Option<TaskScope>,
    status: Option<TaskStatus>,
}

async fn list(
    State(state): State<AppState>,
    context: RequestContext,
    ApiQuery(query): ApiQuery<ListTasksQuery>,
) -> AppResult<Json<ListTasksResponse>> {
    Ok(Json(
        tasks::list(
            state.pool(),
            context.workspace_id,
            query.scope,
            query.status,
            state.clock().now(),
            context.timezone,
        )
        .await?,
    ))
}

async fn create(
    State(state): State<AppState>,
    context: RequestContext,
    ApiJson(request): ApiJson<CreateTaskRequest>,
) -> AppResult<(StatusCode, Json<Task>)> {
    let task = tasks::create(
        state.pool(),
        context.workspace_id,
        context.user_id,
        request,
        state.clock().now().timestamp_millis(),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(task)))
}

async fn get_one(
    State(state): State<AppState>,
    context: RequestContext,
    Path(uid): Path<String>,
) -> AppResult<Json<Task>> {
    Ok(Json(
        tasks::get(state.pool(), context.workspace_id, &uid).await?,
    ))
}

async fn update(
    State(state): State<AppState>,
    context: RequestContext,
    Path(uid): Path<String>,
    ApiJson(request): ApiJson<UpdateTaskRequest>,
) -> AppResult<Json<Task>> {
    Ok(Json(
        tasks::update(
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
    tasks::delete(state.pool(), context.workspace_id, &uid).await?;
    Ok(StatusCode::NO_CONTENT)
}
