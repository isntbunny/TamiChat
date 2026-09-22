use crate::db;
use crate::protocol::{ClientMsg, HistoryItem, ServerMsg};
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

    async fn is_online(&self, user: &str) -> bool {
        self.users.lock().await.contains_key(user)
    }
}

pub async fn handle_socket(socket: WebSocket, username: String, state: AppState) {
    let hub = state.hub.clone();
    let pool = state.pool.clone();

    // 重名检查
    {
        let map = hub.users.lock().await;
        if map.contains_key(&username) {
            return;
        }
    }

    let (mut ws_tx, mut ws_rx) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMsg>();

    {
        let mut map = hub.users.lock().await;
        map.insert(username.clone(), tx);
    }

    // ---- 发送任务 ----
    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let text = match serde_json::to_string(&msg) {
                Ok(t) => t,
                Err(_) => continue,
            };
            if ws_tx.send(Message::Text(text)).await.is_err() {
                break;
            }
        }
    });

    // ---- 上线后：推送离线消息 ----
    let pool_clone = pool.clone();
    let username_clone = username.clone();
    let hub_clone = hub.clone();
    tokio::spawn(async move {
        if let Ok(msgs) = db::take_undelivered(&pool_clone, &username_clone).await {
            for m in msgs {
                let _ = hub_clone
                    .send_to(
                        &username_clone,
                        ServerMsg::NewMessage {
                            from: m.from_user,
                            content: m.content,
                            to_me: true,
                            created_at: m.created_at,
                        },
                    )
                    .await;
            }
        }
    });

    // ---- 广播上线 ----
    hub.broadcast_online().await;
    {
        let sys = ServerMsg::System {
            content: format!("{} 上线了", username),
        };
        let map = hub.users.lock().await;
        for (name, tx) in map.iter() {
            if name != &username {
                let _ = tx.send(sys.clone());
            }
        }
    }

    // ---- 接收任务 ----
    let hub_recv = hub.clone();
    let pool_recv = pool.clone();
    let username_recv = username.clone();
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
                    let peer_online = hub_recv.is_online(&to).await;

                    // 存库（记下送达状态）
                    let now = chrono::Utc::now().to_rfc3339();
                    let _ = db::save_message(
                        &pool_recv,
                        &username_recv,
                        &to,
                        &content,
                        peer_online,
                    )
                    .await;

                    if peer_online {
                        // 推给对方
                        let _ = hub_recv
                            .send_to(
                                &to,
                                ServerMsg::NewMessage {
                                    from: username_recv.clone(),
                                    content: content.clone(),
                                    to_me: true,
                                    created_at: now.clone(),
                                },
                            )
                            .await;

                        // 回显给自己
                        let _ = hub_recv
                            .send_to(
                                &username_recv,
                                ServerMsg::NewMessage {
                                    from: username_recv.clone(),
                                    content,
                                    to_me: false,
                                    created_at: now,
                                },
                            )
                            .await;
                    } else {
                        // 对方离线：只回显给自己，对方上线时会收到
                        let _ = hub_recv
                            .send_to(
                                &username_recv,
                                ServerMsg::NewMessage {
                                    from: username_recv.clone(),
                                    content,
                                    to_me: false,
                                    created_at: now,
                                },
                            )
                            .await;
                    }
                }

                ClientMsg::LoadHistory { with } => {
                    if let Ok(rows) =
                        db::history_between(&pool_recv, &username_recv, &with, 100).await
                    {
                        let items: Vec<HistoryItem> = rows
                            .into_iter()
                            .map(|m| HistoryItem {
                                to_me: m.from_user != username_recv,
                                from: m.from_user,
                                content: m.content,
                                created_at: m.created_at,
                            })
                            .collect();

                        let _ = hub_recv
                            .send_to(
                                &username_recv,
                                ServerMsg::History {
                                    with,
                                    messages: items,
                                },
                            )
                            .await;
                    }
                }
            }
        }
    });

    // ---- 等待任一任务结束 ----
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    // ---- 清理 + 广播离线 ----
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