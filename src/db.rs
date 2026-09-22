use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

// ==================== 消息模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StoredMessage {
    pub id: i64,
    pub from_user: String,
    pub to_user: String,
    pub content: String,
    pub created_at: String,
    pub delivered: i64, // 0 = 未送达, 1 = 已送达
}

// ==================== 用户模型 ====================

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserRow {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
}

// ==================== 初始化 ====================

pub async fn init_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite:tamichat.db?mode=rwc")
        .await
        .expect("无法打开数据库");

    // 消息表
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
        "#,
    )
    .execute(&pool)
    .await
    .expect("建 messages 表失败");

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_to_user ON messages(to_user, delivered);
        "#,
    )
    .execute(&pool)
    .await
    .expect("建 messages 索引失败");

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_pair ON messages(from_user, to_user);
        "#,
    )
    .execute(&pool)
    .await
    .expect("建 messages 索引失败");

    // 用户表
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            username      TEXT    NOT NULL UNIQUE,
            password_hash TEXT    NOT NULL,
            created_at    TEXT    NOT NULL
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("建 users 表失败");

    pool
}

// ==================== 消息相关 ====================

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

// ==================== 用户相关 ====================

/// 创建用户，用户名冲突时返回错误
pub async fn create_user(
    pool: &SqlitePool,
    username: &str,
    password_hash: &str,
) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    let res = sqlx::query(
        "INSERT INTO users (username, password_hash, created_at) VALUES (?, ?, ?)",
    )
    .bind(username)
    .bind(password_hash)
    .bind(&now)
    .execute(pool)
    .await;

    match res {
        Ok(_) => Ok(()),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            Err("用户名已被占用".into())
        }
        Err(e) => Err(format!("数据库错误: {}", e)),
    }
}

/// 根据用户名查用户
pub async fn find_user(
    pool: &SqlitePool,
    username: &str,
) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "SELECT id, username, password_hash FROM users WHERE username = ?",
    )
    .bind(username)
    .fetch_optional(pool)
    .await
}