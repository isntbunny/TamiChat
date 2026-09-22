mod auth;
mod db;
mod hub;
mod protocol;

use axum::{
    extract::{ws::WebSocketUpgrade, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use auth::AuthBody;
use hub::Hub;
use sqlx::SqlitePool;
use std::sync::Arc;
use tower_http::services::ServeDir;

#[derive(Clone)]
pub struct AppState {
    pub hub: Arc<Hub>,
    pub pool: SqlitePool,
}

#[tokio::main]
async fn main() {
    let pool = db::init_pool().await;
    let state = AppState {
        hub: Arc::new(Hub::new()),
        pool,
    };

    let app = Router::new()
        .route("/api/register", post(api_register))
        .route("/api/login", post(api_login))
        .route("/ws", get(ws_handler))
        .fallback_service(ServeDir::new("static"))
        .with_state(state);

    let addr = "0.0.0.0:3000";
    println!("TamiChat running at http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// ==================== 注册 ====================

async fn api_register(
    State(state): State<AppState>,
    Json(body): Json<AuthBody>,
) -> impl IntoResponse {
    match auth::register(&state.pool, body).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err((code, msg)) => (
            StatusCode::from_u16(code).unwrap_or(StatusCode::BAD_REQUEST),
            Json(serde_json::json!({ "error": msg })),
        )
            .into_response(),
    }
}

// ==================== 登录 ====================

async fn api_login(
    State(state): State<AppState>,
    Json(body): Json<AuthBody>,
) -> impl IntoResponse {
    match auth::login(&state.pool, body).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err((code, msg)) => (
            StatusCode::from_u16(code).unwrap_or(StatusCode::UNAUTHORIZED),
            Json(serde_json::json!({ "error": msg })),
        )
            .into_response(),
    }
}

// ==================== WebSocket ====================

#[derive(serde::Deserialize)]
struct WsQuery {
    token: String,
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(q): Query<WsQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let username = match auth::verify_token(&q.token) {
        Some(u) => u,
        None => {
            return (StatusCode::UNAUTHORIZED, "invalid token").into_response();
        }
    };

    ws.on_upgrade(move |socket| hub::handle_socket(socket, username, state))
}