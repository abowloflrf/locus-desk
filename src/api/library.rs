use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::Deserialize;

use crate::{
    api::{ApiJson, ApiQuery},
    error::AppResult,
    library::{
        self, CreateLibraryItemRequest, LibraryContent, LibraryItem, LibraryStatus,
        ListLibraryItemsOptions, ListLibraryItemsResponse, UpdateLibraryItemRequest,
    },
    state::AppState,
    workspace::RequestContext,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/library", get(list).post(create))
        .route("/library/{uid}", get(get_one).patch(update).delete(delete))
        .route("/library/{uid}/content", get(get_content))
        .route("/library/{uid}/retry", post(retry_fetch))
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ListLibraryItemsQuery {
    status: LibraryStatus,
    q: Option<String>,
    tag: Option<String>,
    read: Option<bool>,
    starred: Option<bool>,
    page: u32,
    page_size: u32,
}

impl Default for ListLibraryItemsQuery {
    fn default() -> Self {
        Self {
            status: LibraryStatus::Active,
            q: None,
            tag: None,
            read: None,
            starred: None,
            page: 1,
            page_size: 30,
        }
    }
}

async fn list(
    State(state): State<AppState>,
    context: RequestContext,
    ApiQuery(query): ApiQuery<ListLibraryItemsQuery>,
) -> AppResult<Json<ListLibraryItemsResponse>> {
    Ok(Json(
        library::list(
            state.pool(),
            context.workspace_id,
            ListLibraryItemsOptions {
                status: query.status,
                query: query.q.as_deref(),
                tag: query.tag.as_deref(),
                read: query.read,
                starred: query.starred,
                page: query.page,
                page_size: query.page_size,
            },
        )
        .await?,
    ))
}

async fn create(
    State(state): State<AppState>,
    context: RequestContext,
    ApiJson(request): ApiJson<CreateLibraryItemRequest>,
) -> AppResult<(StatusCode, Json<LibraryItem>)> {
    let outcome = library::create(
        state.pool(),
        context.workspace_id,
        context.user_id,
        request,
        state.clock().now().timestamp_millis(),
    )
    .await?;
    let status = if outcome.created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(outcome.item)))
}

async fn get_one(
    State(state): State<AppState>,
    context: RequestContext,
    Path(uid): Path<String>,
) -> AppResult<Json<LibraryItem>> {
    Ok(Json(
        library::get(state.pool(), context.workspace_id, &uid).await?,
    ))
}

async fn get_content(
    State(state): State<AppState>,
    context: RequestContext,
    Path(uid): Path<String>,
) -> AppResult<Json<LibraryContent>> {
    Ok(Json(
        library::get_content(state.pool(), context.workspace_id, &uid).await?,
    ))
}

async fn retry_fetch(
    State(state): State<AppState>,
    context: RequestContext,
    Path(uid): Path<String>,
) -> AppResult<(StatusCode, Json<LibraryItem>)> {
    Ok((
        StatusCode::ACCEPTED,
        Json(
            library::retry_fetch(
                state.pool(),
                context.workspace_id,
                &uid,
                state.clock().now().timestamp_millis(),
            )
            .await?,
        ),
    ))
}

async fn update(
    State(state): State<AppState>,
    context: RequestContext,
    Path(uid): Path<String>,
    ApiJson(request): ApiJson<UpdateLibraryItemRequest>,
) -> AppResult<Json<LibraryItem>> {
    Ok(Json(
        library::update(
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
    library::delete(state.pool(), context.workspace_id, &uid).await?;
    Ok(StatusCode::NO_CONTENT)
}
