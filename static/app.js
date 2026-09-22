let ws = null;
let me = "";
let token = "";
let currentPeer = null;
let onlineUsers = [];

// ==================== 认证 ====================

async function doAuth(mode) {
  const username = document.getElementById("username").value.trim();
  const password = document.getElementById("password").value;
  const errBox = document.getElementById("auth-error");
  errBox.textContent = "";

  if (!username || !password) {
    errBox.textContent = "请填写用户名和密码";
    return;
  }

  try {
    const res = await fetch(`/api/${mode}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ username, password }),
    });
    const data = await res.json();

    if (!res.ok) {
      errBox.textContent = data.error || "失败";
      return;
    }

    token = data.token;
    me = data.username;
    connectWs();
  } catch (e) {
    errBox.textContent = "网络错误";
  }
}

function connectWs() {
  ws = new WebSocket(
    `ws://${location.host}/ws?token=${encodeURIComponent(token)}`,
  );

  ws.onopen = () => {
    document.getElementById("login").classList.add("hidden");
    document.getElementById("app").classList.remove("hidden");
    document.getElementById("me-name").textContent = me;
    document.getElementById("me-avatar").textContent = me[0].toUpperCase();
    document.getElementById("input").disabled = true;
  };

  ws.onmessage = (e) => handle(JSON.parse(e.data));

  ws.onclose = () => {
    alert("连接已断开，请重新登录");
    location.reload();
  };
}

// ==================== 消息分发 ====================

function handle(msg) {
  switch (msg.type) {
    case "OnlineUsers":
      onlineUsers = msg.users;
      renderDmList();
      break;
    case "NewMessage":
      onNewMessage(msg);
      break;
    case "History":
      renderHistory(msg.with, msg.messages);
      break;
    case "System":
      // 可选：显示系统提示
      break;
    case "Error":
      console.warn("服务端错误:", msg.content);
      break;
  }
}

// ==================== 会话列表 ====================

function renderDmList() {
  const box = document.getElementById("dm-list");
  box.innerHTML = "";

  const others = onlineUsers.filter((u) => u !== me);

  if (others.length === 0) {
    const empty = document.createElement("div");
    empty.className = "group-title";
    empty.style.padding = "8px";
    empty.textContent = "暂无其他用户在线";
    box.appendChild(empty);
    return;
  }

  others.forEach((u) => {
    const item = document.createElement("div");
    item.className = "channel-item dm";
    if (u === currentPeer) item.classList.add("active");
    item.innerHTML = `
      <span class="avatar-sm">${u[0].toUpperCase()}</span>
      <span class="name">${escapeHtml(u)}</span>
      <span class="dot online"></span>
    `;
    item.onclick = () => selectPeer(u);
    box.appendChild(item);
  });
}

// ==================== 选中某个用户 ====================

function selectPeer(peer) {
  currentPeer = peer;
  document.getElementById("chat-title").textContent = `与 ${peer} 聊天`;
  document.getElementById("messages").innerHTML = "";
  document.getElementById("input").disabled = false;
  document.querySelector("#send-form button").disabled = false;
  document.getElementById("input").focus();

  // 刷新列表高亮
  renderDmList();

  // 拉历史
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: "LoadHistory", with: peer }));
  }
}

// ==================== 渲染 ====================

function onNewMessage(msg) {
  // msg: { from, content, to_me, created_at }
  const from = msg.from;
  const isMe = !msg.to_me;

  // 判断这条消息属于哪个会话
  const peer = isMe ? currentPeer : from;

  // 如果正在看这个会话，立刻渲染
  if (peer === currentPeer) {
    appendMessage(msg);
  } else {
    // 不在这里，可以在列表上标未读，这里先略
  }
}

function appendMessage(msg) {
  const isMe = !msg.to_me;
  const box = document.getElementById("messages");
  const div = document.createElement("div");
  div.className = "msg-group" + (isMe ? " me" : "");
  div.innerHTML = `
    <div class="msg-avatar">${msg.from[0].toUpperCase()}</div>
    <div class="msg-body">
      <div class="msg-head">
        <span class="msg-author">${escapeHtml(msg.from)}</span>
        <span class="msg-time">${formatTime(msg.created_at)}</span>
      </div>
      <div class="msg-text">${escapeHtml(msg.content)}</div>
    </div>
  `;
  box.appendChild(div);
  box.scrollTop = box.scrollHeight;
}

function renderHistory(peer, messages) {
  if (peer !== currentPeer) return;
  const box = document.getElementById("messages");
  box.innerHTML = "";
  messages.forEach((m) => {
    // 历史里 m.to_me = true 表示别人发给我
    appendMessage({
      from: m.from,
      content: m.content,
      to_me: m.to_me,
      created_at: m.created_at,
    });
  });
}

// ==================== 发送 ====================

document.getElementById("send-form").addEventListener("submit", (e) => {
  e.preventDefault();
  const input = document.getElementById("input");
  const content = input.value.trim();
  if (!content || !currentPeer || !ws) return;

  ws.send(
    JSON.stringify({
      type: "SendMessage",
      to: currentPeer,
      content: content,
    }),
  );
  input.value = "";
});

// ==================== 工具 ====================

function formatTime(iso) {
  try {
    const d = new Date(iso);
    const hh = String(d.getHours()).padStart(2, "0");
    const mm = String(d.getMinutes()).padStart(2, "0");
    return `${hh}:${mm}`;
  } catch {
    return "";
  }
}

function escapeHtml(s) {
  const d = document.createElement("div");
  d.textContent = s;
  return d.innerHTML;
}

// 回车登录
document.addEventListener("DOMContentLoaded", () => {
  const pw = document.getElementById("password");
  if (pw) {
    pw.addEventListener("keydown", (e) => {
      if (e.key === "Enter") doAuth("login");
    });
  }
});
