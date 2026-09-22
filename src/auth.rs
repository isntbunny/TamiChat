use crate::db;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// 开发用密钥，正式项目请改成环境变量
const JWT_SECRET: &[u8] = b"tamichat-dev-secret-change-me";

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // username
    pub exp: usize,
}

// ==================== 密码哈希 ====================

pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let parsed = match PasswordHash::new(hash) {
        Ok(p) => p,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

// ==================== JWT ====================

pub fn make_token(username: &str) -> Result<String, String> {
    let exp = (chrono::Utc::now() + chrono::Duration::days(7)).timestamp() as usize;
    let claims = Claims {
        sub: username.to_string(),
        exp,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(JWT_SECRET),
    )
    .map_err(|e| e.to_string())
}

pub fn verify_token(token: &str) -> Option<String> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(JWT_SECRET),
        &Validation::default(),
    )
    .ok()?;
    Some(data.claims.sub)
}

// ==================== HTTP 请求/响应 ====================

#[derive(Deserialize)]
pub struct AuthBody {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub username: String,
}

// ==================== 注册 ====================

pub async fn register(
    pool: &SqlitePool,
    body: AuthBody,
) -> Result<AuthResponse, (u16, String)> {
    let username = body.username.trim();
    if username.len() < 2 || username.len() > 20 {
        return Err((400, "用户名长度需在 2-20 之间".into()));
    }
    if body.password.len() < 6 {
        return Err((400, "密码至少 6 位".into()));
    }

    let hash = hash_password(&body.password).map_err(|e| (500, e))?;
    db::create_user(pool, username, &hash)
        .await
        .map_err(|e| (400, e))?;

    let token = make_token(username).map_err(|e| (500, e))?;
    Ok(AuthResponse {
        token,
        username: username.to_string(),
    })
}

// ==================== 登录 ====================

pub async fn login(
    pool: &SqlitePool,
    body: AuthBody,
) -> Result<AuthResponse, (u16, String)> {
    let username = body.username.trim();
    let user = db::find_user(pool, username)
        .await
        .map_err(|e| (500, e.to_string()))?
        .ok_or((401, "用户名或密码错误".to_string()))?;

    if !verify_password(&body.password, &user.password_hash) {
        return Err((401, "用户名或密码错误".into()));
    }

    let token = make_token(username).map_err(|e| (500, e))?;
    Ok(AuthResponse {
        token,
        username: username.to_string(),
    })
}