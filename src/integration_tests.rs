use std::sync::Arc;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, Response, StatusCode, header},
};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;
use ulid::Ulid;

use crate::{app, auth, clock::FixedClock, config::Config, state::AppState};

const TEST_ORIGIN: &str = "http://locus.test";
const TEST_HOST: &str = "locus.test";
const TEST_PASSWORD: &str = "correct horse battery staple";

struct TestApp {
    _directory: TempDir,
    state: AppState,
    router: Router,
}

impl TestApp {
    async fn new() -> Self {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let now = DateTime::parse_from_rfc3339("2026-08-23T16:30:00Z")
            .expect("test time should be valid")
            .with_timezone(&Utc);
        let config = Config::for_test(
            directory.path().join("data"),
            "admin",
            TEST_PASSWORD,
            "Asia/Singapore".parse().expect("timezone should be valid"),
        );
        let state = AppState::initialize_with_clock(config, Arc::new(FixedClock::new(now)))
            .await
            .expect("test application should initialize");
        let router = app(state.clone());
        Self {
            _directory: directory,
            state,
            router,
        }
    }

    async fn request(
        &self,
        method: Method,
        uri: &str,
        cookie: Option<&str>,
        body: Option<Value>,
    ) -> Response<Body> {
        let mut builder = Request::builder().method(method.clone()).uri(uri);
        builder = builder.header(header::HOST, TEST_HOST);
        if !matches!(method, Method::GET | Method::HEAD | Method::OPTIONS) {
            builder = builder.header(header::ORIGIN, TEST_ORIGIN);
        }
        if let Some(cookie) = cookie {
            builder = builder.header(header::COOKIE, cookie);
        }
        let body = if let Some(body) = body {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Body::from(serde_json::to_vec(&body).expect("request JSON should serialize"))
        } else {
            Body::empty()
        };
        self.router
            .clone()
            .oneshot(builder.body(body).expect("request should be valid"))
            .await
            .expect("router should respond")
    }

    async fn login(&self) -> (String, Value) {
        let response = self
            .request(
                Method::POST,
                "/api/v1/auth/login",
                None,
                Some(json!({"username": "admin", "password": TEST_PASSWORD})),
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let cookie = response
            .headers()
            .get(header::SET_COOKIE)
            .expect("login should set a cookie")
            .to_str()
            .expect("cookie should be text")
            .split(';')
            .next()
            .expect("cookie should have a value")
            .to_owned();
        (cookie, response_json(response).await)
    }
}

async fn response_json(response: Response<Body>) -> Value {
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("response body should be readable");
    serde_json::from_slice(&body).expect("response body should be JSON")
}

#[tokio::test]
async fn phase_one_migration_backfills_object_identity_without_losing_data() {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .foreign_keys(true),
        )
        .await
        .expect("migration fixture should open");
    sqlx::raw_sql(include_str!("../migrations/0001_initial.sql"))
        .execute(&pool)
        .await
        .expect("schema v1 should migrate");
    for statement in [
        "INSERT INTO users (id, uid, username, password_hash, created_at, updated_at) VALUES (1, 'user', 'owner', 'hash', 1, 1)",
        "INSERT INTO workspaces (id, uid, name, timezone, created_by, created_at, updated_at) VALUES (1, 'workspace', 'Personal', 'UTC', 1, 1, 1)",
        "INSERT INTO workspace_members (workspace_id, user_id, role, created_at) VALUES (1, 1, 'OWNER', 1)",
        "INSERT INTO notes (id, uid, workspace_id, creator_id, content, status, pinned, created_at, updated_at) VALUES (11, 'note', 1, 1, 'Keep me #saved', 'ACTIVE', 1, 2, 3)",
        "INSERT INTO note_tags (note_id, tag) VALUES (11, 'saved')",
        "INSERT INTO tasks (id, uid, workspace_id, creator_id, title, description, status, priority, sort_key, created_at, updated_at) VALUES (12, 'task', 1, 1, 'Keep this task', '', 'TODO', 0, 0, 4, 5)",
    ] {
        sqlx::query(statement)
            .execute(&pool)
            .await
            .expect("schema v1 fixture should be inserted");
    }

    sqlx::raw_sql(include_str!("../migrations/0002_phase1_library.sql"))
        .execute(&pool)
        .await
        .expect("schema v2 should migrate");

    let objects = sqlx::query_as::<_, (String, String)>(
        "SELECT uid, object_type FROM objects ORDER BY object_type ASC",
    )
    .fetch_all(&pool)
    .await
    .expect("backfilled objects should be readable");
    assert_eq!(
        objects,
        vec![
            ("note".to_owned(), "NOTE".to_owned()),
            ("task".to_owned(), "TASK".to_owned()),
        ]
    );
    let note = sqlx::query_as::<_, (String, String, bool)>(
        "SELECT uid, content, pinned FROM notes WHERE id = 11",
    )
    .fetch_one(&pool)
    .await
    .expect("the migrated memo should remain readable");
    assert_eq!(note, ("note".to_owned(), "Keep me #saved".to_owned(), true));
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT object_tags.tag FROM object_tags JOIN notes ON notes.object_id = object_tags.object_id WHERE notes.uid = 'note'",
        )
        .fetch_one(&pool)
        .await
        .expect("memo tags should be backfilled into the object layer"),
        "saved"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT title FROM tasks WHERE id = 12")
            .fetch_one(&pool)
            .await
            .expect("the migrated task should remain readable"),
        "Keep this task"
    );
    assert!(
        sqlx::query(
            "INSERT INTO notes (uid, workspace_id, creator_id, content, created_at, updated_at) VALUES ('invalid', 1, 1, 'Missing object', 1, 1)",
        )
        .execute(&pool)
        .await
        .is_err(),
        "new memos must carry a non-null object identity"
    );
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .expect("foreign key check should run")
            .is_empty()
    );
}

#[tokio::test]
async fn phase_one_upgrade_creates_a_recoverable_schema_one_backup() {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let config = Config::for_test(
        directory.path().join("data"),
        "admin",
        TEST_PASSWORD,
        "Asia/Singapore".parse().expect("timezone should be valid"),
    );
    crate::db::prepare_data_directories(&config).expect("data directories should be prepared");
    let pool = crate::db::connect(&config)
        .await
        .expect("schema v1 database should open");
    MIGRATOR
        .run_to(1, &pool)
        .await
        .expect("only schema v1 should migrate");
    crate::db::bootstrap(&pool, &config, 1_000)
        .await
        .expect("schema v1 owner should bootstrap");
    let (user_id, workspace_id) = sqlx::query_as::<_, (i64, i64)>(
        r#"
        SELECT users.id, workspaces.id
        FROM users
        JOIN workspaces ON workspaces.created_by = users.id
        WHERE users.username = 'admin'
        "#,
    )
    .fetch_one(&pool)
    .await
    .expect("owner workspace should exist");
    sqlx::query(
        r#"
        INSERT INTO notes
          (uid, workspace_id, creator_id, content, status, pinned, created_at, updated_at)
        VALUES ('pre_upgrade_note', ?, ?, 'Before Phase 1', 'ACTIVE', 0, 1100, 1100)
        "#,
    )
    .bind(workspace_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .expect("schema v1 memo should be inserted");
    pool.close().await;

    let upgrade_time = DateTime::parse_from_rfc3339("2026-08-24T01:00:00Z")
        .expect("upgrade time should parse")
        .with_timezone(&Utc);
    let upgraded =
        AppState::initialize_with_clock(config.clone(), Arc::new(FixedClock::new(upgrade_time)))
            .await
            .expect("schema v1 should back up and migrate to the latest schema");
    assert_eq!(crate::db::schema_version(upgraded.pool()).await.unwrap(), 3);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM objects WHERE uid = 'pre_upgrade_note' AND object_type = 'NOTE'",
        )
        .fetch_one(upgraded.pool())
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM jobs")
            .fetch_one(upgraded.pool())
            .await
            .unwrap(),
        0,
        "a schema v1 database without Library items must not gain orphan jobs"
    );

    let backup = std::fs::read_dir(config.backups_dir())
        .expect("backup directory should be readable")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("pre-migration-") && name.ends_with("-schema-1.sqlite3")
                })
        })
        .expect("upgrade should create a schema v1 snapshot");
    let restore_root = directory.path().join("restored-v1");
    let restore_db = restore_root.join("db/locus-desk.sqlite3");
    std::fs::create_dir_all(restore_db.parent().unwrap())
        .expect("restore database directory should be created");
    crate::data_management::restore_sqlite_backup(&backup, &restore_root, &restore_db)
        .await
        .expect("pre-migration snapshot should be recoverable");
    let restored = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&restore_db)
                .read_only(true)
                .create_if_missing(false),
        )
        .await
        .expect("restored schema v1 database should open");
    assert_eq!(crate::db::schema_version(&restored).await.unwrap(), 1);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT content FROM notes WHERE uid = 'pre_upgrade_note'",
        )
        .fetch_one(&restored)
        .await
        .unwrap(),
        "Before Phase 1"
    );
}

#[tokio::test]
async fn bootstraps_authenticates_and_reports_real_schema() {
    let application = TestApp::new().await;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let data_mode = std::fs::metadata(application.state.config().data_dir())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        let database_mode = std::fs::metadata(application.state.config().database_path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(data_mode, 0o700);
        assert_eq!(database_mode, 0o600);
    }

    let health = application
        .request(Method::GET, "/api/v1/health", None, None)
        .await;
    assert_eq!(health.status(), StatusCode::OK);
    assert_eq!(
        health.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );
    assert!(health.headers().contains_key("x-request-id"));
    let health = response_json(health).await;
    assert_eq!(health["schemaVersion"], 3);

    let bootstrap = application
        .request(Method::GET, "/api/v1/bootstrap/status", None, None)
        .await;
    assert_eq!(response_json(bootstrap).await["initialized"], true);

    let original_hash =
        sqlx::query_scalar::<_, String>("SELECT password_hash FROM users WHERE username = 'admin'")
            .fetch_one(application.state.pool())
            .await
            .unwrap();
    let repeat_config = Config::for_test(
        application._directory.path().join("data"),
        "replacement",
        "different bootstrap password",
        "UTC".parse().unwrap(),
    );
    let repeat_clock = DateTime::parse_from_rfc3339("2026-08-24T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let repeated =
        AppState::initialize_with_clock(repeat_config, Arc::new(FixedClock::new(repeat_clock)))
            .await
            .unwrap();
    let users = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
        .fetch_one(repeated.pool())
        .await
        .unwrap();
    let unchanged_hash =
        sqlx::query_scalar::<_, String>("SELECT password_hash FROM users WHERE username = 'admin'")
            .fetch_one(repeated.pool())
            .await
            .unwrap();
    assert_eq!(users, 1);
    assert_eq!(unchanged_hash, original_hash);

    let blank_username = application
        .request(
            Method::POST,
            "/api/v1/auth/login",
            None,
            Some(json!({"username": "   ", "password": "not relevant"})),
        )
        .await;
    assert_eq!(blank_username.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(blank_username).await["error"]["code"],
        "invalid_request"
    );

    let too_long_username = application
        .request(
            Method::POST,
            "/api/v1/auth/login",
            None,
            Some(json!({
                "username": "界".repeat(101),
                "password": "not relevant"
            })),
        )
        .await;
    assert_eq!(too_long_username.status(), StatusCode::BAD_REQUEST);

    let wrong_password = application
        .request(
            Method::POST,
            "/api/v1/auth/login",
            None,
            Some(json!({"username": "admin", "password": "wrong password"})),
        )
        .await;
    assert_eq!(wrong_password.status(), StatusCode::UNAUTHORIZED);
    let wrong_password_error = response_json(wrong_password).await;

    let missing_user = application
        .request(
            Method::POST,
            "/api/v1/auth/login",
            None,
            Some(json!({"username": "missing-user", "password": "wrong password"})),
        )
        .await;
    assert_eq!(missing_user.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(response_json(missing_user).await, wrong_password_error);

    let oversized_password = "x".repeat(auth::MAX_PASSWORD_BYTES + 1);
    for username in ["admin", "another-missing-user"] {
        let response = application
            .request(
                Method::POST,
                "/api/v1/auth/login",
                None,
                Some(json!({"username": username, "password": oversized_password})),
            )
            .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(response_json(response).await, wrong_password_error);
    }

    let oversized_login_body = application
        .request(
            Method::POST,
            "/api/v1/auth/login",
            None,
            Some(json!({"username": "admin", "password": "x".repeat(9 * 1024)})),
        )
        .await;
    assert_eq!(oversized_login_body.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(oversized_login_body).await["error"]["code"],
        "invalid_request"
    );

    let (cookie, session) = application.login().await;
    assert_eq!(session["user"]["username"], "admin");
    assert_eq!(session["workspace"]["today"], "2026-08-24");
    assert_eq!(session["workspace"]["role"], "OWNER");

    let me = application
        .request(Method::GET, "/api/v1/auth/me", Some(&cookie), None)
        .await;
    assert_eq!(me.status(), StatusCode::OK);

    let logout = application
        .request(Method::POST, "/api/v1/auth/logout", Some(&cookie), None)
        .await;
    assert_eq!(logout.status(), StatusCode::NO_CONTENT);
    let me = application
        .request(Method::GET, "/api/v1/auth/me", Some(&cookie), None)
        .await;
    assert_eq!(me.status(), StatusCode::UNAUTHORIZED);

    let (expired_cookie, _) = application.login().await;
    let expired_token = expired_cookie
        .strip_prefix("locus_session=")
        .expect("session cookie should contain a token");
    sqlx::query("UPDATE sessions SET expires_at = 0 WHERE token_hash = ?")
        .bind(auth::hash_token(expired_token))
        .execute(application.state.pool())
        .await
        .unwrap();
    let expired = application
        .request(Method::GET, "/api/v1/auth/me", Some(&expired_cookie), None)
        .await;
    assert_eq!(expired.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn note_crud_search_tags_pin_and_archive_are_persistent() {
    let application = TestApp::new().await;
    let (cookie, _) = application.login().await;

    let created = application
        .request(
            Method::POST,
            "/api/v1/notes",
            Some(&cookie),
            Some(json!({"content": "中文 note 100% #Rust #学习\n`#ignored`"})),
        )
        .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created = response_json(created).await;
    let uid = created["uid"].as_str().unwrap();
    assert_eq!(created["tags"], json!(["rust", "学习"]));

    for uri in [
        "/api/v1/notes?q=%E4%B8%AD%E6%96%87",
        "/api/v1/notes?q=%25",
        "/api/v1/notes?tag=rust",
    ] {
        let response = application
            .request(Method::GET, uri, Some(&cookie), None)
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response_json(response).await["total"], 1);
    }

    let tags = application
        .request(Method::GET, "/api/v1/tags", Some(&cookie), None)
        .await;
    assert_eq!(response_json(tags).await["items"], json!(["rust", "学习"]));

    let updated = application
        .request(
            Method::PATCH,
            &format!("/api/v1/notes/{uid}"),
            Some(&cookie),
            Some(json!({
                "content": "Updated #next",
                "pinned": true,
                "status": "ARCHIVED"
            })),
        )
        .await;
    assert_eq!(updated.status(), StatusCode::OK);
    let updated = response_json(updated).await;
    assert_eq!(updated["tags"], json!(["next"]));
    assert_eq!(updated["pinned"], true);

    let active = application
        .request(Method::GET, "/api/v1/notes", Some(&cookie), None)
        .await;
    assert_eq!(response_json(active).await["total"], 0);
    let archived = application
        .request(
            Method::GET,
            "/api/v1/notes?status=ARCHIVED",
            Some(&cookie),
            None,
        )
        .await;
    assert_eq!(response_json(archived).await["total"], 1);

    let deleted = application
        .request(
            Method::DELETE,
            &format!("/api/v1/notes/{uid}"),
            Some(&cookie),
            None,
        )
        .await;
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);

    let invalid = application
        .request(
            Method::POST,
            "/api/v1/notes",
            Some(&cookie),
            Some(json!({"content": "   "})),
        )
        .await;
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn library_capture_deduplicates_urls_and_preserves_each_capture() {
    let application = TestApp::new().await;
    let (cookie, _) = application.login().await;

    let created = application
        .request(
            Method::POST,
            "/api/v1/library",
            Some(&cookie),
            Some(json!({
                "url": " HTTPS://Example.COM:443/a/../article?x=1#first ",
                "title": "First title",
                "selection": "First passage",
                "note": "Read this",
                "tags": ["Rust", "reading"],
                "idempotencyKey": "capture-first"
            })),
        )
        .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created = response_json(created).await;
    let uid = created["uid"].as_str().unwrap().to_owned();
    assert_eq!(created["normalizedUrl"], "https://example.com/article?x=1");
    assert_eq!(created["siteName"], "example.com");
    assert_eq!(created["tags"], json!(["reading", "rust"]));
    assert_eq!(created["captures"].as_array().unwrap().len(), 1);
    assert_eq!(created["processingStatus"], "PENDING");
    assert_eq!(created["contentAvailable"], false);
    assert_eq!(created["contentVersion"], 0);

    let pending_content = application
        .request(
            Method::GET,
            &format!("/api/v1/library/{uid}/content"),
            Some(&cookie),
            None,
        )
        .await;
    assert_eq!(pending_content.status(), StatusCode::CONFLICT);
    let pending_retry = application
        .request(
            Method::POST,
            &format!("/api/v1/library/{uid}/retry"),
            Some(&cookie),
            None,
        )
        .await;
    assert_eq!(pending_retry.status(), StatusCode::ACCEPTED);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM jobs WHERE status IN ('PENDING', 'RUNNING', 'RETRY')",
        )
        .fetch_one(application.state.pool())
        .await
        .unwrap(),
        1,
        "retrying a pending item must not duplicate its active job"
    );

    let duplicate = application
        .request(
            Method::POST,
            "/api/v1/library",
            Some(&cookie),
            Some(json!({
                "url": "https://example.com/article?x=1#second",
                "title": "A later page title",
                "selection": "Second passage",
                "note": "Keep the second context",
                "tags": ["Research"],
                "idempotencyKey": "capture-second"
            })),
        )
        .await;
    assert_eq!(duplicate.status(), StatusCode::OK);
    let duplicate = response_json(duplicate).await;
    assert_eq!(duplicate["uid"], uid);
    assert_eq!(duplicate["captures"].as_array().unwrap().len(), 2);
    assert_eq!(duplicate["captures"][1]["selectedText"], "Second passage");
    assert_eq!(duplicate["tags"], json!(["reading", "research", "rust"]));

    let retried = application
        .request(
            Method::POST,
            "/api/v1/library",
            Some(&cookie),
            Some(json!({
                "url": "https://example.com/article?x=1",
                "selection": "A retry must not append this",
                "idempotencyKey": "capture-second"
            })),
        )
        .await;
    assert_eq!(retried.status(), StatusCode::OK);
    let retried = response_json(retried).await;
    assert_eq!(retried["uid"], uid);
    assert_eq!(retried["captures"].as_array().unwrap().len(), 2);

    let conflict = application
        .request(
            Method::POST,
            "/api/v1/library",
            Some(&cookie),
            Some(json!({
                "url": "https://example.com/different",
                "idempotencyKey": "capture-second"
            })),
        )
        .await;
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    assert_eq!(response_json(conflict).await["error"]["code"], "conflict");

    let searched = application
        .request(
            Method::GET,
            "/api/v1/library?q=Second%20passage&tag=research",
            Some(&cookie),
            None,
        )
        .await;
    assert_eq!(searched.status(), StatusCode::OK);
    assert_eq!(response_json(searched).await["total"], 1);

    let updated = application
        .request(
            Method::PATCH,
            &format!("/api/v1/library/{uid}"),
            Some(&cookie),
            Some(json!({
                "title": "Saved article",
                "read": true,
                "starred": true,
                "tags": ["saved"]
            })),
        )
        .await;
    assert_eq!(updated.status(), StatusCode::OK);
    let updated = response_json(updated).await;
    assert_eq!(updated["title"], "Saved article");
    assert!(updated["readAt"].is_string());
    assert_eq!(updated["starred"], true);
    assert_eq!(updated["tags"], json!(["saved"]));

    let filtered = application
        .request(
            Method::GET,
            "/api/v1/library?read=true&starred=true",
            Some(&cookie),
            None,
        )
        .await;
    assert_eq!(response_json(filtered).await["total"], 1);

    let archived = application
        .request(
            Method::PATCH,
            &format!("/api/v1/library/{uid}"),
            Some(&cookie),
            Some(json!({"status": "ARCHIVED"})),
        )
        .await;
    assert_eq!(archived.status(), StatusCode::OK);
    assert_eq!(
        response_json(
            application
                .request(Method::GET, "/api/v1/library", Some(&cookie), None)
                .await
        )
        .await["total"],
        0
    );
    assert_eq!(
        response_json(
            application
                .request(
                    Method::GET,
                    "/api/v1/library?status=ARCHIVED",
                    Some(&cookie),
                    None,
                )
                .await
        )
        .await["total"],
        1
    );

    for body in [
        json!({"url": "ftp://example.com/file"}),
        json!({"url": "https://user:secret@example.com/"}),
        json!({"url": "   "}),
    ] {
        let invalid = application
            .request(Method::POST, "/api/v1/library", Some(&cookie), Some(body))
            .await;
        assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    let (item_id, object_id) = sqlx::query_as::<_, (i64, i64)>(
        r#"
        SELECT li.id, li.object_id
        FROM library_items li
        JOIN objects o
          ON o.id = li.object_id AND o.workspace_id = li.workspace_id
        WHERE o.uid = ? AND o.object_type = 'LIBRARY_ITEM'
        "#,
    )
    .bind(&uid)
    .fetch_one(application.state.pool())
    .await
    .expect("Library identity should exist before deletion");

    let deleted = application
        .request(
            Method::DELETE,
            &format!("/api/v1/library/{uid}"),
            Some(&cookie),
            None,
        )
        .await;
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
    let missing = application
        .request(
            Method::GET,
            &format!("/api/v1/library/{uid}"),
            Some(&cookie),
            None,
        )
        .await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);

    for (table, query, id) in [
        (
            "object",
            "SELECT COUNT(*) FROM objects WHERE id = ?",
            object_id,
        ),
        (
            "item",
            "SELECT COUNT(*) FROM library_items WHERE id = ?",
            item_id,
        ),
        (
            "capture",
            "SELECT COUNT(*) FROM library_captures WHERE library_item_id = ?",
            item_id,
        ),
        (
            "tag",
            "SELECT COUNT(*) FROM object_tags WHERE object_id = ?",
            object_id,
        ),
        (
            "job",
            "SELECT COUNT(*) FROM jobs WHERE object_id = ?",
            object_id,
        ),
    ] {
        let remaining = sqlx::query_scalar::<_, i64>(query)
            .bind(id)
            .fetch_one(application.state.pool())
            .await
            .unwrap_or_else(|error| panic!("deleted Library {table} should be queryable: {error}"));
        assert_eq!(remaining, 0, "deleted Library {table} should be removed");
    }
}

#[tokio::test]
async fn concurrent_library_idempotency_creates_one_item_and_capture() {
    let application = TestApp::new().await;
    let (cookie, _) = application.login().await;
    let payload = json!({
        "url": "https://example.com/concurrent#fragment",
        "selection": "Only once",
        "idempotencyKey": "concurrent-capture"
    });

    let (left, right) = tokio::join!(
        application.request(
            Method::POST,
            "/api/v1/library",
            Some(&cookie),
            Some(payload.clone())
        ),
        application.request(
            Method::POST,
            "/api/v1/library",
            Some(&cookie),
            Some(payload)
        )
    );
    assert!(
        left.status().is_success(),
        "left request: {}",
        left.status()
    );
    assert!(
        right.status().is_success(),
        "right request: {}",
        right.status()
    );
    let left = response_json(left).await;
    let right = response_json(right).await;
    assert_eq!(left["uid"], right["uid"]);
    assert_eq!(left["captures"].as_array().unwrap().len(), 1);
    assert_eq!(right["captures"].as_array().unwrap().len(), 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM library_items")
            .fetch_one(application.state.pool())
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM library_captures")
            .fetch_one(application.state.pool())
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM jobs")
            .fetch_one(application.state.pool())
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn today_tasks_handle_overdue_completion_restore_and_null_schedule() {
    let application = TestApp::new().await;
    let (cookie, _) = application.login().await;

    let overdue = application
        .request(
            Method::POST,
            "/api/v1/tasks",
            Some(&cookie),
            Some(json!({
                "title": "Overdue",
                "priority": 1,
                "dueDate": "2026-08-23",
                "dueTime": "09:30"
            })),
        )
        .await;
    assert_eq!(overdue.status(), StatusCode::CREATED);
    let overdue = response_json(overdue).await;
    let overdue_uid = overdue["uid"].as_str().unwrap();

    for body in [
        json!({"title": "Today", "dueDate": "2026-08-24"}),
        json!({"title": "Inbox"}),
        json!({"title": "Future", "dueDate": "2026-08-25"}),
    ] {
        assert_eq!(
            application
                .request(Method::POST, "/api/v1/tasks", Some(&cookie), Some(body),)
                .await
                .status(),
            StatusCode::CREATED
        );
    }

    let today = application
        .request(
            Method::GET,
            "/api/v1/tasks?scope=today",
            Some(&cookie),
            None,
        )
        .await;
    let today = response_json(today).await;
    assert_eq!(today["items"].as_array().unwrap().len(), 2);
    assert_eq!(today["items"][0]["title"], "Overdue");

    let completed = application
        .request(
            Method::PATCH,
            &format!("/api/v1/tasks/{overdue_uid}"),
            Some(&cookie),
            Some(json!({"status": "DONE"})),
        )
        .await;
    let completed = response_json(completed).await;
    assert_eq!(completed["status"], "DONE");
    assert!(completed["completedAt"].is_string());

    let today = application
        .request(
            Method::GET,
            "/api/v1/tasks?scope=today",
            Some(&cookie),
            None,
        )
        .await;
    let today = response_json(today).await;
    assert_eq!(today["items"].as_array().unwrap().len(), 2);
    assert_eq!(today["items"][1]["status"], "DONE");

    let restored = application
        .request(
            Method::PATCH,
            &format!("/api/v1/tasks/{overdue_uid}"),
            Some(&cookie),
            Some(json!({"status": "TODO", "dueDate": null, "dueTime": null})),
        )
        .await;
    let restored = response_json(restored).await;
    assert_eq!(restored["status"], "TODO");
    assert_eq!(restored["completedAt"], Value::Null);
    assert_eq!(restored["dueDate"], Value::Null);
    assert_eq!(restored["dueTime"], Value::Null);

    let invalid = application
        .request(
            Method::POST,
            "/api/v1/tasks",
            Some(&cookie),
            Some(json!({"title": "Invalid", "dueTime": "09:30"})),
        )
        .await;
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn patch_rejects_null_for_non_nullable_note_and_task_fields() {
    let application = TestApp::new().await;
    let (cookie, _) = application.login().await;
    let note = application
        .request(
            Method::POST,
            "/api/v1/notes",
            Some(&cookie),
            Some(json!({"content": "Original"})),
        )
        .await;
    let note_uid = response_json(note).await["uid"]
        .as_str()
        .unwrap()
        .to_owned();
    for field in ["content", "status", "pinned"] {
        let response = application
            .request(
                Method::PATCH,
                &format!("/api/v1/notes/{note_uid}"),
                Some(&cookie),
                Some(json!({(field): null})),
            )
            .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "field: {field}");
        assert_eq!(
            response_json(response).await["error"]["code"],
            "invalid_request"
        );
    }

    let task = application
        .request(
            Method::POST,
            "/api/v1/tasks",
            Some(&cookie),
            Some(json!({
                "title": "Original",
                "dueDate": "2026-08-24",
                "dueTime": "09:30"
            })),
        )
        .await;
    let task_uid = response_json(task).await["uid"]
        .as_str()
        .unwrap()
        .to_owned();
    for field in ["title", "description", "status", "priority", "sortKey"] {
        let response = application
            .request(
                Method::PATCH,
                &format!("/api/v1/tasks/{task_uid}"),
                Some(&cookie),
                Some(json!({(field): null})),
            )
            .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "field: {field}");
        assert_eq!(
            response_json(response).await["error"]["code"],
            "invalid_request"
        );
    }

    let cleared = application
        .request(
            Method::PATCH,
            &format!("/api/v1/tasks/{task_uid}"),
            Some(&cookie),
            Some(json!({"dueDate": null, "dueTime": null})),
        )
        .await;
    assert_eq!(cleared.status(), StatusCode::OK);
    let cleared = response_json(cleared).await;
    assert_eq!(cleared["dueDate"], Value::Null);
    assert_eq!(cleared["dueTime"], Value::Null);

    let library = application
        .request(
            Method::POST,
            "/api/v1/library",
            Some(&cookie),
            Some(json!({"url": "https://example.com/null-patch"})),
        )
        .await;
    let library_uid = response_json(library).await["uid"]
        .as_str()
        .unwrap()
        .to_owned();
    for field in ["title", "status", "read", "starred", "tags"] {
        let response = application
            .request(
                Method::PATCH,
                &format!("/api/v1/library/{library_uid}"),
                Some(&cookie),
                Some(json!({(field): null})),
            )
            .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "field: {field}");
        assert_eq!(
            response_json(response).await["error"]["code"],
            "invalid_request"
        );
    }
}

#[tokio::test]
async fn concurrent_task_patches_preserve_unrelated_fields() {
    let application = TestApp::new().await;
    let (cookie, _) = application.login().await;

    for index in 0..12 {
        let created = application
            .request(
                Method::POST,
                "/api/v1/tasks",
                Some(&cookie),
                Some(json!({"title": format!("Original {index}")})),
            )
            .await;
        let uid = response_json(created).await["uid"]
            .as_str()
            .unwrap()
            .to_owned();
        let path = format!("/api/v1/tasks/{uid}");
        let expected_title = format!("Renamed {index}");
        let title_patch = application.request(
            Method::PATCH,
            &path,
            Some(&cookie),
            Some(json!({"title": expected_title})),
        );
        let completion_patch = application.request(
            Method::PATCH,
            &path,
            Some(&cookie),
            Some(json!({"status": "DONE"})),
        );
        let (title_response, completion_response) = tokio::join!(title_patch, completion_patch);
        assert_eq!(title_response.status(), StatusCode::OK);
        assert_eq!(completion_response.status(), StatusCode::OK);

        let task = application
            .request(Method::GET, &path, Some(&cookie), None)
            .await;
        let task = response_json(task).await;
        assert_eq!(task["title"], format!("Renamed {index}"));
        assert_eq!(task["status"], "DONE");
        assert!(task["completedAt"].is_string());
    }
}

#[tokio::test]
async fn concurrent_schedule_patches_return_a_stable_validation_error() {
    let application = TestApp::new().await;
    let (cookie, _) = application.login().await;

    for index in 0..16 {
        let created = application
            .request(
                Method::POST,
                "/api/v1/tasks",
                Some(&cookie),
                Some(json!({
                    "title": format!("Scheduled {index}"),
                    "dueDate": "2026-08-24"
                })),
            )
            .await;
        let uid = response_json(created).await["uid"]
            .as_str()
            .unwrap()
            .to_owned();
        let path = format!("/api/v1/tasks/{uid}");
        let time_patch = application.request(
            Method::PATCH,
            &path,
            Some(&cookie),
            Some(json!({"dueTime": "09:30"})),
        );
        let date_patch = application.request(
            Method::PATCH,
            &path,
            Some(&cookie),
            Some(json!({"dueDate": null})),
        );
        let (time_response, date_response) = tokio::join!(time_patch, date_patch);
        let time_succeeded = time_response.status() == StatusCode::OK;
        let mut statuses = [
            time_response.status().as_u16(),
            date_response.status().as_u16(),
        ];
        statuses.sort_unstable();
        assert_eq!(
            statuses,
            [
                StatusCode::OK.as_u16(),
                StatusCode::UNPROCESSABLE_ENTITY.as_u16()
            ]
        );

        let rejected = if time_response.status() == StatusCode::UNPROCESSABLE_ENTITY {
            time_response
        } else {
            date_response
        };
        assert_eq!(
            response_json(rejected).await["error"]["code"],
            "validation_failed"
        );

        let task = response_json(
            application
                .request(Method::GET, &path, Some(&cookie), None)
                .await,
        )
        .await;
        if time_succeeded {
            assert_eq!(task["dueDate"], "2026-08-24");
            assert_eq!(task["dueTime"], "09:30");
        } else {
            assert_eq!(task["dueDate"], Value::Null);
            assert_eq!(task["dueTime"], Value::Null);
        }
    }
}

#[tokio::test]
async fn workspace_isolation_and_request_boundaries_return_json_errors() {
    let application = TestApp::new().await;
    let (cookie, _) = application.login().await;
    let note = application
        .request(
            Method::POST,
            "/api/v1/notes",
            Some(&cookie),
            Some(json!({"content": "Private"})),
        )
        .await;
    let note_uid = response_json(note).await["uid"]
        .as_str()
        .unwrap()
        .to_owned();
    let task = application
        .request(
            Method::POST,
            "/api/v1/tasks",
            Some(&cookie),
            Some(json!({"title": "Private task"})),
        )
        .await;
    let task_uid = response_json(task).await["uid"]
        .as_str()
        .unwrap()
        .to_owned();
    let library = application
        .request(
            Method::POST,
            "/api/v1/library",
            Some(&cookie),
            Some(json!({"url": "https://example.com/private"})),
        )
        .await;
    let library_uid = response_json(library).await["uid"]
        .as_str()
        .unwrap()
        .to_owned();

    let user_id = sqlx::query_scalar::<_, i64>("SELECT id FROM users WHERE username = 'admin'")
        .fetch_one(application.state.pool())
        .await
        .unwrap();
    let workspace_uid = Ulid::generate().to_string();
    let workspace = sqlx::query(
        "INSERT INTO workspaces (uid, name, timezone, created_by, created_at, updated_at) VALUES (?, 'Other', 'Asia/Singapore', ?, 0, 0)",
    )
    .bind(workspace_uid)
    .bind(user_id)
    .execute(application.state.pool())
    .await
    .unwrap();
    let workspace_id = workspace.last_insert_rowid();
    sqlx::query(
        "INSERT INTO workspace_members (workspace_id, user_id, role, created_at) VALUES (?, ?, 'OWNER', 0)",
    )
    .bind(workspace_id)
    .bind(user_id)
    .execute(application.state.pool())
    .await
    .unwrap();
    let raw_token = cookie
        .strip_prefix("locus_session=")
        .expect("test cookie should contain session token");
    sqlx::query("UPDATE sessions SET active_workspace_id = ? WHERE token_hash = ?")
        .bind(workspace_id)
        .bind(auth::hash_token(raw_token))
        .execute(application.state.pool())
        .await
        .unwrap();

    let hidden = application
        .request(
            Method::GET,
            &format!("/api/v1/notes/{note_uid}"),
            Some(&cookie),
            None,
        )
        .await;
    assert_eq!(hidden.status(), StatusCode::NOT_FOUND);
    let hidden_update = application
        .request(
            Method::PATCH,
            &format!("/api/v1/notes/{note_uid}"),
            Some(&cookie),
            Some(json!({"content": "Blocked"})),
        )
        .await;
    assert_eq!(hidden_update.status(), StatusCode::NOT_FOUND);
    let hidden_delete = application
        .request(
            Method::DELETE,
            &format!("/api/v1/notes/{note_uid}"),
            Some(&cookie),
            None,
        )
        .await;
    assert_eq!(hidden_delete.status(), StatusCode::NOT_FOUND);
    let empty_notes = application
        .request(Method::GET, "/api/v1/notes", Some(&cookie), None)
        .await;
    assert_eq!(response_json(empty_notes).await["total"], 0);

    let hidden_task = application
        .request(
            Method::GET,
            &format!("/api/v1/tasks/{task_uid}"),
            Some(&cookie),
            None,
        )
        .await;
    assert_eq!(hidden_task.status(), StatusCode::NOT_FOUND);
    let hidden_task_update = application
        .request(
            Method::PATCH,
            &format!("/api/v1/tasks/{task_uid}"),
            Some(&cookie),
            Some(json!({"title": "Blocked"})),
        )
        .await;
    assert_eq!(hidden_task_update.status(), StatusCode::NOT_FOUND);
    let hidden_task_delete = application
        .request(
            Method::DELETE,
            &format!("/api/v1/tasks/{task_uid}"),
            Some(&cookie),
            None,
        )
        .await;
    assert_eq!(hidden_task_delete.status(), StatusCode::NOT_FOUND);
    let empty_tasks = application
        .request(Method::GET, "/api/v1/tasks", Some(&cookie), None)
        .await;
    assert!(
        response_json(empty_tasks).await["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let hidden_library = application
        .request(
            Method::GET,
            &format!("/api/v1/library/{library_uid}"),
            Some(&cookie),
            None,
        )
        .await;
    assert_eq!(hidden_library.status(), StatusCode::NOT_FOUND);
    let hidden_library_update = application
        .request(
            Method::PATCH,
            &format!("/api/v1/library/{library_uid}"),
            Some(&cookie),
            Some(json!({"starred": true})),
        )
        .await;
    assert_eq!(hidden_library_update.status(), StatusCode::NOT_FOUND);
    let hidden_library_content = application
        .request(
            Method::GET,
            &format!("/api/v1/library/{library_uid}/content"),
            Some(&cookie),
            None,
        )
        .await;
    assert_eq!(hidden_library_content.status(), StatusCode::NOT_FOUND);
    let hidden_library_retry = application
        .request(
            Method::POST,
            &format!("/api/v1/library/{library_uid}/retry"),
            Some(&cookie),
            None,
        )
        .await;
    assert_eq!(hidden_library_retry.status(), StatusCode::NOT_FOUND);
    let hidden_library_delete = application
        .request(
            Method::DELETE,
            &format!("/api/v1/library/{library_uid}"),
            Some(&cookie),
            None,
        )
        .await;
    assert_eq!(hidden_library_delete.status(), StatusCode::NOT_FOUND);
    let empty_library = application
        .request(Method::GET, "/api/v1/library", Some(&cookie), None)
        .await;
    assert_eq!(response_json(empty_library).await["total"], 0);
    let same_url_other_workspace = application
        .request(
            Method::POST,
            "/api/v1/library",
            Some(&cookie),
            Some(json!({"url": "https://example.com/private"})),
        )
        .await;
    assert_eq!(same_url_other_workspace.status(), StatusCode::CREATED);
    let same_url_other_workspace = response_json(same_url_other_workspace).await;
    assert_ne!(
        same_url_other_workspace["uid"].as_str().unwrap(),
        library_uid
    );

    let cross_origin = application
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/notes")
                .header(header::HOST, TEST_HOST)
                .header(header::ORIGIN, "https://attacker.test")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"content":"Blocked"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cross_origin.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response_json(cross_origin).await["error"]["code"],
        "origin_not_allowed"
    );

    for origin in [None, Some("https://locus.test")] {
        let mut request = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/notes")
            .header(header::HOST, TEST_HOST)
            .header(header::COOKIE, &cookie)
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(origin) = origin {
            request = request.header(header::ORIGIN, origin);
        }
        let response = application
            .router
            .clone()
            .oneshot(
                request
                    .body(Body::from(r#"{"content":"Blocked"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response_json(response).await["error"]["code"],
            "origin_not_allowed"
        );
    }

    let unknown = application
        .request(Method::GET, "/api/v1/unknown", None, None)
        .await;
    assert_eq!(unknown.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        unknown.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/json"
    );

    let method_not_allowed = application
        .request(Method::POST, "/api/v1/health", None, None)
        .await;
    assert_eq!(method_not_allowed.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        response_json(method_not_allowed).await["error"]["code"],
        "method_not_allowed"
    );

    let missing_content_type = application
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/notes")
                .header(header::HOST, TEST_HOST)
                .header(header::ORIGIN, TEST_ORIGIN)
                .header(header::COOKIE, &cookie)
                .body(Body::from(r#"{"content":"Missing type"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_content_type.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        missing_content_type
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap(),
        "application/json"
    );
}
