use crate::protocol::{ClientMsg, ServerMsg};
use crate::AppState;
use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use tokio::sync::{mpsc, Mutex};

pub struct Hub {
    users: Mutex<HashMap<String, mpsc::UnboundedSender<ServerMsg>>>,
}

impl Hub {
    pub fn new() -> Self {
        Self {
            users: Mutex::new(HashMap::new()),
        }
    }

    async fn online_list(&self) -> Vec<String> {
        self.users.lock().await.keys().cloned().collect()
    }

    async fn broadcast_online(&self) {
        let users = self.online_list().await;
        let msg = ServerMsg::OnlineUsers { users };
        let map = self.users.lock().await;
        for tx in map.values() {
            let _ = tx.send(msg.clone());
        }
    }

    async fn send_to(&self, to: &str, msg: ServerMsg) -> bool {
        let map = self.users.lock().await;
        if let Some(tx) = map.get(to) {
            tx.send(msg).is_ok()
        } else {
            false
        }
    }
}

pub async fn handle_socket(socket: WebSocket, username: String, state: AppState) {
    let hub = state.hub.clone();

    // 1. 检查重名
    {
        let map = hub.users.lock().await;
        if map.contains_key(&username) {
            return;
        }
    }

    let (mut ws_tx, mut ws_rx) = socket.split();

    // 2. 注册用户，建立发送通道
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMsg>();
    {
        let mut map = hub.users.lock().await;
        map.insert(username.clone(), tx);
    }

    // 3. 发送任务：把 channel 里的消息写回 WebSocket
    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let text = serde_json::to_string(&msg).unwrap();
            if ws_tx.send(Message::Text(text)).await.is_err() {
                break;
            }
        }
    });

    // 4. 通知所有人：有人上线
    hub.broadcast_online().await;
    let sys = ServerMsg::System {
        content: format!("{} 上线了", username),
    };
    {
        let map = hub.users.lock().await;
        for (name, tx) in map.iter() {
            if name != &username {
                let _ = tx.send(sys.clone());
            }
        }
    }

    // 5. 接收任务：读客户端消息
    let hub_clone = hub.clone();
    let username_clone = username.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            let text = match msg {
                Message::Text(t) => t,
                Message::Close(_) => break,
                _ => continue,
            };

            let client_msg: ClientMsg = match serde_json::from_str(&text) {
                Ok(m) => m,
                Err(_) => continue,
            };

            match client_msg {
                ClientMsg::SendMessage { to, content } => {
                    // 发给对方
                    let delivered = hub_clone
                        .send_to(
                            &to,
                            ServerMsg::NewMessage {
                                from: username_clone.clone(),
                                content: content.clone(),
                                to_me: true,
                            },
                        )
                        .await;

                    if !delivered {
                        // 对方不在线，告诉发送者
                        let _ = hub_clone
                            .send_to(
                                &username_clone,
                                ServerMsg::Error {
                                    content: format!("{} 不在线", to),
                                },
                            )
                            .await;
                    } else {
                        // 给自己也回显一份（to_me = false 表示自己发的）
                        let _ = hub_clone
                            .send_to(
                                &username_clone,
                                ServerMsg::NewMessage {
                                    from: username_clone.clone(),
                                    content,
                                    to_me: false,
                                },
                            )
                            .await;
                    }
                }
            }
        }
    });

    // 6. 任一任务结束，清理
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    // 7. 移除用户，广播离线
    {
        let mut map = hub.users.lock().await;
        map.remove(&username);
    }
    hub.broadcast_online().await;

    let sys = ServerMsg::System {
        content: format!("{} 下线了", username),
    };
    let map = hub.users.lock().await;
    for (name, tx) in map.iter() {
        if name != &username {
            let _ = tx.send(sys.clone());
        }
    }
}