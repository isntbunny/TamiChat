use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub enum ClientMsg {
    /// 发消息给对方
    SendMessage { to: String, content: String },
}

#[derive(Serialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum ServerMsg {
    /// 在线用户列表更新
    OnlineUsers { users: Vec<String> },
    /// 收到一条新消息
    NewMessage {
        from: String,
        content: String,
        /// 是否是发给自己的（前端用来区分左右）
        to_me: bool,
    },
    /// 系统提示（上线/离线）
    System { content: String },
    /// 错误
    Error { content: String },
}