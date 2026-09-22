use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub enum ClientMsg {
    SendMessage { to: String, content: String },
    /// 请求和某人的历史消息
    LoadHistory { with: String },
}

#[derive(Serialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum ServerMsg {
    OnlineUsers { users: Vec<String> },
    NewMessage {
        from: String,
        content: String,
        to_me: bool,
        created_at: String,
    },
    /// 历史消息批量下发
    History {
        with: String,
        messages: Vec<HistoryItem>,
    },
    System { content: String },
    Error { content: String },
}

#[derive(Serialize, Debug, Clone)]
pub struct HistoryItem {
    pub from: String,
    pub content: String,
    pub to_me: bool,
    pub created_at: String,
}