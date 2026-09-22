use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StoredMessage {
    pub id: i64,
    pub from_user: String,
    pub to_user: String,
    pub content: String,
    pub created_at: String,
    pub delivered: i64,   // 0 = 未送达, 1 = 已送达
}

pub async fn init_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite:tamichat.db?mode=rwc")
        .await
        .expect("无法打开数据库");

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS messages (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            from_user   TEXT    NOT NULL,
            to_user     TEXT    NOT NULL,
            content     TEXT    NOT NULL,
            created_at  TEXT    NOT NULL,
            delivered   INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_to_user ON messages(to_user, delivered);
        CREATE INDEX IF NOT EXISTS idx_pair ON messages(from_user, to_user);
        "#,
    )
    .execute(&pool)
    .await
    .expect("建表失败");

    pool
}

/// 存一条消息，返回它的 id
pub async fn save_message(
    pool: &SqlitePool,
    from: &str,
    to: &str,
    content: &str,
    delivered: bool,
) -> Result<i64, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let res = sqlx::query(
        "INSERT INTO messages (from_user, to_user, content, created_at, delivered)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(from)
    .bind(to)
    .bind(content)
    .bind(&now)
    .bind(if delivered { 1 } else { 0 })
    .execute(pool)
    .await?;

    Ok(res.last_insert_rowid())
}

/// 拉取某个用户所有未送达的消息，并标记为已送达
pub async fn take_undelivered(
    pool: &SqlitePool,
    user: &str,
) -> Result<Vec<StoredMessage>, sqlx::Error> {
    let msgs = sqlx::query_as::<_, StoredMessage>(
        "SELECT id, from_user, to_user, content, created_at, delivered
         FROM messages
         WHERE to_user = ? AND delivered = 0
         ORDER BY id ASC",
    )
    .bind(user)
    .fetch_all(pool)
    .await?;

    if !msgs.is_empty() {
        sqlx::query("UPDATE messages SET delivered = 1 WHERE to_user = ? AND delivered = 0")
            .bind(user)
            .execute(pool)
            .await?;
    }

    Ok(msgs)
}

/// 拉取两个用户之间的历史消息（双向）
pub async fn history_between(
    pool: &SqlitePool,
    a: &str,
    b: &str,
    limit: i64,
) -> Result<Vec<StoredMessage>, sqlx::Error> {
    let rows = sqlx::query_as::<_, StoredMessage>(
        "SELECT id, from_user, to_user, content, created_at, delivered
         FROM messages
         WHERE (from_user = ? AND to_user = ?)
            OR (from_user = ? AND to_user = ?)
         ORDER BY id DESC
         LIMIT ?",
    )
    .bind(a)
    .bind(b)
    .bind(b)
    .bind(a)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    // 倒序拉出来，再翻回正序
    Ok(rows.into_iter().rev().collect())
}