mod db;
mod hub;
mod protocol;

use axum::{
    extract::{ws::WebSocketUpgrade, Query, State},
    response::IntoResponse,
    routing::get,
    Router,
};
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
        .route("/ws", get(ws_handler))
        .fallback_service(ServeDir::new("static"))
        .with_state(state);

    let addr = "0.0.0.0:3000";
    println!("TamiChat running at http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(serde::Deserialize)]
struct WsQuery {
    username: String,
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(q): Query<WsQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| hub::handle_socket(socket, q.username, state))
}