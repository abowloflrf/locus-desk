//! Authentication and session domain.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, LazyLock, Mutex},
};

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use axum_extra::extract::cookie::{Cookie, SameSite};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use cookie::time::Duration as CookieDuration;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, SqlitePool};
use tokio::sync::Semaphore;

use crate::error::{AppError, AppResult};

pub const SESSION_COOKIE_NAME: &str = "locus_session";
const SESSION_DURATION_MILLIS: i64 = 30 * 24 * 60 * 60 * 1_000;
const LOGIN_WINDOW_MILLIS: i64 = 60_000;
const MAX_LOGIN_FAILURES: usize = 5;
const MAX_TRACKED_USERNAMES: usize = 128;
const MAX_GLOBAL_LOGIN_FAILURES: usize = 200;
const MAX_USERNAME_LOGIN_IN_FLIGHT: usize = 2;
const MAX_GLOBAL_LOGIN_IN_FLIGHT: usize = 8;
const MAX_CONCURRENT_ARGON2: usize = 2;
pub(crate) const MAX_PASSWORD_BYTES: usize = 1_024;
const DUMMY_PASSWORD_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$NZd0h/3f1rf8cwE+gqP/Rw$9MQBE4tB10Y/bxKo/9O9PWkhhn+tECaeB0uotpzUFi8";
const DUMMY_PASSWORD: &str = "locus-dummy-password";
static ARGON2_SEMAPHORE: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(MAX_CONCURRENT_ARGON2)));

#[derive(Clone, Debug, FromRow)]
pub struct SessionIdentity {
    pub token_hash: String,
    pub user_id: i64,
    pub user_uid: String,
    pub username: String,
    pub workspace_id: i64,
    pub workspace_uid: String,
    pub workspace_name: String,
    pub timezone: String,
    pub role: String,
    pub expires_at: i64,
}

#[derive(FromRow)]
struct LoginAccount {
    user_id: i64,
    user_uid: String,
    username: String,
    password_hash: String,
    workspace_id: i64,
    workspace_uid: String,
    workspace_name: String,
    timezone: String,
    role: String,
}

#[derive(Default)]
struct LoginBucket {
    attempts: VecDeque<i64>,
    in_flight: usize,
    last_seen: i64,
}

#[derive(Default)]
struct LoginLimiterState {
    usernames: HashMap<String, LoginBucket>,
    global_attempts: VecDeque<i64>,
    in_flight: usize,
}

#[derive(Default)]
pub struct LoginLimiter {
    state: Mutex<LoginLimiterState>,
}

#[must_use = "login reservations must be completed or held until the attempt ends"]
pub struct LoginReservation<'a> {
    limiter: &'a LoginLimiter,
    username: String,
    reserved_at: i64,
    active: bool,
}

impl LoginLimiter {
    pub fn reserve(&self, username: &str, now: i64) -> AppResult<LoginReservation<'_>> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| AppError::Internal("login limiter lock was poisoned".to_owned()))?;
        prune_limiter(&mut state, now);
        if state.global_attempts.len() >= MAX_GLOBAL_LOGIN_FAILURES
            || state.in_flight >= MAX_GLOBAL_LOGIN_IN_FLIGHT
        {
            return Err(AppError::rate_limited());
        }
        let key = limiter_key(username);
        if state.usernames.get(&key).is_some_and(|bucket| {
            bucket.attempts.len() >= MAX_LOGIN_FAILURES
                || bucket.in_flight >= MAX_USERNAME_LOGIN_IN_FLIGHT
        }) {
            return Err(AppError::rate_limited());
        }
        if !state.usernames.contains_key(&key)
            && state.usernames.len() >= MAX_TRACKED_USERNAMES
            && !evict_oldest_inactive_username(&mut state.usernames)
        {
            return Err(AppError::rate_limited());
        }
        state.in_flight += 1;
        let reservation_username = key.clone();
        let bucket = state.usernames.entry(key).or_default();
        bucket.in_flight += 1;
        bucket.last_seen = now;
        Ok(LoginReservation {
            limiter: self,
            username: reservation_username,
            reserved_at: now,
            active: true,
        })
    }
}

impl LoginReservation<'_> {
    pub fn succeed(mut self, now: i64) -> AppResult<()> {
        self.finish(now, true)
    }

    pub fn fail(mut self, now: i64) -> AppResult<()> {
        self.finish(now, false)
    }

    fn finish(&mut self, now: i64, succeeded: bool) -> AppResult<()> {
        let mut state = self
            .limiter
            .state
            .lock()
            .map_err(|_| AppError::Internal("login limiter lock was poisoned".to_owned()))?;
        prune_limiter(&mut state, now);
        release_reservation(&mut state, &self.username);

        if succeeded {
            if let Some(bucket) = state.usernames.get_mut(&self.username) {
                bucket.attempts.clear();
                bucket.last_seen = now;
            }
        } else {
            record_login_failure(&mut state, &self.username, now);
        }
        remove_empty_bucket(&mut state.usernames, &self.username);
        self.active = false;
        Ok(())
    }
}

impl Drop for LoginReservation<'_> {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        if let Ok(mut state) = self.limiter.state.lock() {
            prune_limiter(&mut state, self.reserved_at);
            release_reservation(&mut state, &self.username);
            record_login_failure(&mut state, &self.username, self.reserved_at);
        }
    }
}

fn record_login_failure(state: &mut LoginLimiterState, username: &str, now: i64) {
    if state.global_attempts.len() < MAX_GLOBAL_LOGIN_FAILURES {
        state.global_attempts.push_back(now);
    }
    let bucket = state.usernames.entry(username.to_owned()).or_default();
    bucket.last_seen = now;
    if bucket.attempts.len() < MAX_LOGIN_FAILURES {
        bucket.attempts.push_back(now);
    }
}

pub fn canonical_username(value: &str) -> AppResult<String> {
    let canonical = value.trim();
    if !(1..=100).contains(&canonical.chars().count()) {
        return Err(AppError::bad_request(
            "username must contain between 1 and 100 characters",
        ));
    }
    Ok(canonical.to_owned())
}

fn limiter_key(username: &str) -> String {
    username.to_lowercase()
}

fn prune_limiter(state: &mut LoginLimiterState, now: i64) {
    discard_old_attempts(&mut state.global_attempts, now);
    state.usernames.retain(|_, bucket| {
        discard_old_attempts(&mut bucket.attempts, now);
        !bucket.attempts.is_empty() || bucket.in_flight > 0
    });
}

fn discard_old_attempts(attempts: &mut VecDeque<i64>, now: i64) {
    while attempts
        .front()
        .is_some_and(|attempt| now.saturating_sub(*attempt) >= LOGIN_WINDOW_MILLIS)
    {
        attempts.pop_front();
    }
}

fn evict_oldest_inactive_username(usernames: &mut HashMap<String, LoginBucket>) -> bool {
    let oldest = usernames
        .iter()
        .filter(|(_, bucket)| bucket.in_flight == 0)
        .min_by_key(|(_, bucket)| bucket.last_seen)
        .map(|(username, _)| username.clone());
    if let Some(username) = oldest {
        usernames.remove(&username);
        true
    } else {
        false
    }
}

fn release_reservation(state: &mut LoginLimiterState, username: &str) {
    debug_assert!(state.in_flight > 0);
    state.in_flight = state.in_flight.saturating_sub(1);
    if let Some(bucket) = state.usernames.get_mut(username) {
        debug_assert!(bucket.in_flight > 0);
        bucket.in_flight = bucket.in_flight.saturating_sub(1);
    }
}

fn remove_empty_bucket(usernames: &mut HashMap<String, LoginBucket>, username: &str) {
    if usernames
        .get(username)
        .is_some_and(|bucket| bucket.attempts.is_empty() && bucket.in_flight == 0)
    {
        usernames.remove(username);
    }
}

pub async fn login(
    pool: &SqlitePool,
    username: &str,
    password: &str,
    now: i64,
) -> AppResult<(String, SessionIdentity)> {
    let account = sqlx::query_as::<_, LoginAccount>(
        r#"
        SELECT
          u.id AS user_id,
          u.uid AS user_uid,
          u.username,
          u.password_hash,
          w.id AS workspace_id,
          w.uid AS workspace_uid,
          w.name AS workspace_name,
          w.timezone,
          wm.role
        FROM users u
        JOIN workspace_members wm ON wm.user_id = u.id
        JOIN workspaces w ON w.id = wm.workspace_id
        WHERE u.username = ?
        ORDER BY w.id ASC
        LIMIT 1
        "#,
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;

    let password_too_long = login_password_too_long(password);
    let password_hash = if password_too_long {
        DUMMY_PASSWORD_HASH
    } else {
        account
            .as_ref()
            .map(|account| account.password_hash.as_str())
            .unwrap_or(DUMMY_PASSWORD_HASH)
    };
    let verification_password = if password_too_long {
        DUMMY_PASSWORD
    } else {
        password
    };
    let password_matches =
        verify_password(verification_password.to_owned(), password_hash.to_owned()).await?;
    let Some(account) = account else {
        return Err(AppError::invalid_credentials());
    };
    if password_too_long || !password_matches {
        return Err(AppError::invalid_credentials());
    }

    let token = generate_token()?;
    let token_hash = hash_token(&token);
    let expires_at = now.saturating_add(SESSION_DURATION_MILLIS);
    sqlx::query(
        "INSERT INTO sessions (token_hash, user_id, active_workspace_id, created_at, expires_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&token_hash)
    .bind(account.user_id)
    .bind(account.workspace_id)
    .bind(now)
    .bind(expires_at)
    .execute(pool)
    .await?;

    Ok((
        token,
        SessionIdentity {
            token_hash,
            user_id: account.user_id,
            user_uid: account.user_uid,
            username: account.username,
            workspace_id: account.workspace_id,
            workspace_uid: account.workspace_uid,
            workspace_name: account.workspace_name,
            timezone: account.timezone,
            role: account.role,
            expires_at,
        },
    ))
}

fn login_password_too_long(password: &str) -> bool {
    password.len() > MAX_PASSWORD_BYTES
}

pub async fn authenticate(pool: &SqlitePool, token: &str, now: i64) -> AppResult<SessionIdentity> {
    let token_hash = hash_token(token);
    let identity = sqlx::query_as::<_, SessionIdentity>(
        r#"
        SELECT
          s.token_hash,
          u.id AS user_id,
          u.uid AS user_uid,
          u.username,
          w.id AS workspace_id,
          w.uid AS workspace_uid,
          w.name AS workspace_name,
          w.timezone,
          wm.role,
          s.expires_at
        FROM sessions s
        JOIN users u ON u.id = s.user_id
        JOIN workspaces w ON w.id = s.active_workspace_id
        JOIN workspace_members wm
          ON wm.workspace_id = s.active_workspace_id AND wm.user_id = s.user_id
        WHERE s.token_hash = ?
        "#,
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await?;

    let Some(identity) = identity else {
        return Err(AppError::unauthorized());
    };
    if identity.expires_at <= now {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(&token_hash)
            .execute(pool)
            .await?;
        return Err(AppError::unauthorized());
    }
    Ok(identity)
}

pub async fn delete_session(pool: &SqlitePool, token_hash: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
        .bind(token_hash)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn cleanup_expired_sessions(pool: &SqlitePool, now: i64) -> AppResult<u64> {
    Ok(sqlx::query("DELETE FROM sessions WHERE expires_at <= ?")
        .bind(now)
        .execute(pool)
        .await?
        .rows_affected())
}

pub fn session_cookie(token: String, secure: bool) -> Cookie<'static> {
    Cookie::build((SESSION_COOKIE_NAME, token))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(secure)
        .max_age(CookieDuration::days(30))
        .build()
}

pub fn removal_cookie() -> Cookie<'static> {
    Cookie::build(SESSION_COOKIE_NAME).path("/").build()
}

pub async fn hash_password(password: String) -> AppResult<String> {
    run_password_work(Arc::clone(&*ARGON2_SEMAPHORE), move || {
        hash_password_sync(&password)
    })
    .await
}

async fn verify_password(password: String, encoded_hash: String) -> AppResult<bool> {
    run_password_work(Arc::clone(&*ARGON2_SEMAPHORE), move || {
        let parsed = PasswordHash::new(&encoded_hash).map_err(|_| AppError::Password)?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    })
    .await
}

async fn run_password_work<T, F>(semaphore: Arc<Semaphore>, operation: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> AppResult<T> + Send + 'static,
{
    let permit = semaphore
        .acquire_owned()
        .await
        .map_err(|_| AppError::Internal("password worker gate was closed".to_owned()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        operation()
    })
    .await
    .map_err(|error| AppError::Internal(format!("password worker failed: {error}")))?
}

fn hash_password_sync(password: &str) -> AppResult<String> {
    let mut salt = [0_u8; 16];
    getrandom::fill(&mut salt).map_err(|_| AppError::Random)?;
    let salt = SaltString::encode_b64(&salt).map_err(|_| AppError::Password)?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| AppError::Password)
}

fn generate_token() -> AppResult<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| AppError::Random)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

pub(crate) fn hash_token(token: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(token.as_bytes()))
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Barrier, mpsc},
        thread,
        time::Duration,
    };

    use axum::http::StatusCode;

    use super::*;

    #[test]
    fn canonical_username_trims_and_enforces_character_limit() {
        assert_eq!(canonical_username("  Admin  ").unwrap(), "Admin");
        assert!(is_bad_request(canonical_username("   ")));
        assert!(canonical_username(&"界".repeat(100)).is_ok());
        assert!(is_bad_request(canonical_username(&"界".repeat(101))));
    }

    #[test]
    fn login_limiter_bounds_and_expires_username_state() {
        let limiter = LoginLimiter::default();
        let now = 10_000;

        for index in 0..MAX_GLOBAL_LOGIN_FAILURES {
            limiter
                .reserve(&format!("failed-{index}"), now)
                .unwrap()
                .fail(now)
                .unwrap();
        }
        {
            let state = limiter.state.lock().unwrap();
            assert_eq!(state.usernames.len(), MAX_TRACKED_USERNAMES);
            assert_eq!(state.global_attempts.len(), MAX_GLOBAL_LOGIN_FAILURES);
            assert_eq!(state.in_flight, 0);
        }
        assert!(is_rate_limited(limiter.reserve("blocked", now)));

        limiter
            .reserve("fresh", now + LOGIN_WINDOW_MILLIS)
            .unwrap()
            .succeed(now + LOGIN_WINDOW_MILLIS)
            .unwrap();
        let state = limiter.state.lock().unwrap();
        assert!(state.usernames.is_empty());
        assert!(state.global_attempts.is_empty());
        assert_eq!(state.in_flight, 0);
    }

    #[test]
    fn login_limiter_preserves_failure_and_success_semantics() {
        let limiter = LoginLimiter::default();
        for _ in 0..MAX_LOGIN_FAILURES {
            limiter
                .reserve("Admin", 1_000)
                .unwrap()
                .fail(1_000)
                .unwrap();
        }
        assert!(is_rate_limited(limiter.reserve("admin", 1_000)));

        let cleared = LoginLimiter::default();
        for _ in 0..(MAX_LOGIN_FAILURES - 1) {
            cleared
                .reserve("Admin", 1_000)
                .unwrap()
                .fail(1_000)
                .unwrap();
        }
        cleared
            .reserve("admin", 1_000)
            .unwrap()
            .succeed(1_000)
            .unwrap();
        cleared
            .reserve("ADMIN", 1_000)
            .unwrap()
            .succeed(1_000)
            .unwrap();
        assert!(cleared.state.lock().unwrap().usernames.is_empty());
    }

    #[test]
    fn login_limiter_atomically_caps_in_flight_attempts() {
        let limiter = LoginLimiter::default();
        let now = 1_000;
        let first = limiter.reserve("Admin", now).unwrap();
        let second = limiter.reserve("admin", now).unwrap();
        assert!(is_rate_limited(limiter.reserve("ADMIN", now)));

        let mut reservations = vec![first, second];
        for index in 0..(MAX_GLOBAL_LOGIN_IN_FLIGHT - MAX_USERNAME_LOGIN_IN_FLIGHT) {
            reservations.push(limiter.reserve(&format!("user-{index}"), now).unwrap());
        }
        assert!(is_rate_limited(limiter.reserve("overflow", now)));

        reservations.pop().unwrap().succeed(now).unwrap();
        reservations.push(limiter.reserve("replacement", now).unwrap());
        for reservation in reservations {
            reservation.succeed(now).unwrap();
        }

        let state = limiter.state.lock().unwrap();
        assert_eq!(state.in_flight, 0);
        assert!(state.usernames.is_empty());
        assert!(state.global_attempts.is_empty());
    }

    #[test]
    fn concurrent_reservations_cannot_overbook_global_capacity() {
        let limiter = Arc::new(LoginLimiter::default());
        let contenders = MAX_GLOBAL_LOGIN_IN_FLIGHT * 4;
        let start = Arc::new(Barrier::new(contenders));
        let reserved = Arc::new(Barrier::new(contenders));

        let successes = thread::scope(|scope| {
            let mut workers = Vec::with_capacity(contenders);
            for index in 0..contenders {
                let limiter = Arc::clone(&limiter);
                let start = Arc::clone(&start);
                let reserved = Arc::clone(&reserved);
                workers.push(scope.spawn(move || {
                    start.wait();
                    let reservation = limiter.reserve(&format!("user-{index}"), 1_000);
                    reserved.wait();
                    match reservation {
                        Ok(reservation) => {
                            reservation.succeed(1_000).unwrap();
                            true
                        }
                        Err(_) => false,
                    }
                }));
            }
            workers
                .into_iter()
                .map(|worker| worker.join().unwrap())
                .filter(|succeeded| *succeeded)
                .count()
        });

        assert_eq!(successes, MAX_GLOBAL_LOGIN_IN_FLIGHT);
        let state = limiter.state.lock().unwrap();
        assert_eq!(state.in_flight, 0);
        assert!(state.usernames.is_empty());
    }

    #[test]
    fn cancelled_login_reservations_count_as_failures() {
        let limiter = LoginLimiter::default();
        let now = 1_000;

        for _ in 0..MAX_LOGIN_FAILURES {
            drop(limiter.reserve("cancelled", now).unwrap());
        }

        assert!(is_rate_limited(limiter.reserve("cancelled", now)));
        let state = limiter.state.lock().unwrap();
        assert_eq!(state.in_flight, 0);
        assert_eq!(state.global_attempts.len(), MAX_LOGIN_FAILURES);
        assert_eq!(
            state.usernames["cancelled"].attempts.len(),
            MAX_LOGIN_FAILURES
        );
    }

    #[test]
    fn login_password_limit_counts_bytes() {
        assert!(!login_password_too_long(&"a".repeat(MAX_PASSWORD_BYTES)));
        assert!(login_password_too_long(&"a".repeat(MAX_PASSWORD_BYTES + 1)));
        assert!(login_password_too_long(
            &"界".repeat(MAX_PASSWORD_BYTES / "界".len() + 1)
        ));
    }

    #[test]
    fn session_cookie_has_the_required_security_attributes() {
        let secure = session_cookie("secret".to_owned(), true);
        assert_eq!(secure.path(), Some("/"));
        assert_eq!(secure.http_only(), Some(true));
        assert_eq!(secure.same_site(), Some(SameSite::Lax));
        assert_eq!(secure.secure(), Some(true));
        assert_eq!(secure.max_age(), Some(CookieDuration::days(30)));

        let local = session_cookie("secret".to_owned(), false);
        assert_eq!(local.secure(), Some(false));
    }

    #[tokio::test]
    async fn password_worker_holds_permit_until_cancelled_work_really_finishes() {
        let semaphore = Arc::new(Semaphore::new(1));
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker_semaphore = Arc::clone(&semaphore);
        let worker = tokio::spawn(async move {
            run_password_work(worker_semaphore, move || {
                let _ = started_tx.send(());
                release_rx
                    .recv_timeout(Duration::from_secs(2))
                    .map_err(|error| AppError::Internal(error.to_string()))?;
                Ok(())
            })
            .await
        });

        started_rx.await.unwrap();
        worker.abort();
        let _ = worker.await;
        assert_eq!(semaphore.available_permits(), 0);

        release_tx.send(()).unwrap();
        for _ in 0..100 {
            if semaphore.available_permits() == 1 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(semaphore.available_permits(), 1);
    }

    fn is_bad_request<T>(result: AppResult<T>) -> bool {
        matches!(
            result,
            Err(AppError::Client {
                status: StatusCode::BAD_REQUEST,
                code: "invalid_request",
                ..
            })
        )
    }

    fn is_rate_limited<T>(result: AppResult<T>) -> bool {
        matches!(
            result,
            Err(AppError::Client {
                status: StatusCode::TOO_MANY_REQUESTS,
                code: "rate_limited",
                ..
            })
        )
    }
}
