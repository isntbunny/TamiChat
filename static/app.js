let ws = null;
let me = "";
let currentPeer = null;

function login() {
  const name = document.getElementById("username").value.trim();
  if (!name) return;
  me = name;

  ws = new WebSocket(`ws://${location.host}/ws?username=${encodeURIComponent(name)}`);

  ws.onopen = () => {
    document.getElementById("login").classList.add("hidden");
    document.getElementById("chat").classList.remove("hidden");
    document.getElementById("me").textContent = me;
  };

  ws.onmessage = (e) => {
    const msg = JSON.parse(e.data);
    handle(msg);
  };

  ws.onclose = () => {
    alert("连接已断开");
    location.reload();
  };
}

function handle(msg) {
  switch (msg.type) {
    case "OnlineUsers":
      renderUsers(msg.users);
      break;
    case "NewMessage":
      renderMessage(msg);
      break;
    case "System":
      renderSystem(msg.content);
      break;
    case "Error":
      renderError(msg.content);
      break;
  }
}

function renderUsers(users) {
  const ul = document.getElementById("users");
  ul.innerHTML = "";
  users.filter(u => u !== me).forEach(u => {
    const li = document.createElement("li");
    li.textContent = u;
    li.className = u === currentPeer ? "active" : "";
    li.onclick = () => selectPeer(u);
    ul.appendChild(li);
  });
}

function selectPeer(peer) {
  currentPeer = peer;
  document.getElementById("peer").textContent = `与 ${peer} 聊天`;
  document.getElementById("messages").innerHTML = "";
  document.getElementById("input").disabled = false;
  document.querySelector("#send-form button").disabled = false;
  document.getElementById("input").focus();
  renderUsersRefresh();
}

function renderUsersRefresh() {
  document.querySelectorAll("#users li").forEach(li => {
    li.classList.toggle("active", li.textContent === currentPeer);
  });
}

document.getElementById("send-form").addEventListener("submit", (e) => {
  e.preventDefault();
  const input = document.getElementById("input");
  const content = input.value.trim();
  if (!content || !currentPeer || !ws) return;

  ws.send(JSON.stringify({
    type: "SendMessage",
    to: currentPeer,
    content: content
  }));
  input.value = "";
});

function renderMessage(msg) {
  // 只显示和当前 peer 相关的消息
  const relevant = msg.to_me
    ? msg.from === currentPeer        // 对方发给我
    : currentPeer === msg.from || currentPeer === undefined; // 我发的

  // 自己发的消息：from 是自己，to_me = false
  const isMe = !msg.to_me && msg.from === me;
  const other = msg.to_me ? msg.from : currentPeer;

  if (msg.to_me && msg.from !== currentPeer) return; // 不是当前会话，忽略
  if (isMe && currentPeer === null) return;

  const div = document.createElement("div");
  div.className = "msg " + (isMe ? "me" : "them");
  div.innerHTML = `<div class="meta">${msg.from}</div>${escapeHtml(msg.content)}`;
  const box = document.getElementById("messages");
  box.appendChild(div);
  box.scrollTop = box.scrollHeight;
}

function renderSystem(text) {
  const div = document.createElement("div");
  div.className = "system";
  div.textContent = text;
  const box = document.getElementById("messages");
  box.appendChild(div);
  box.scrollTop = box.scrollHeight;
}

function renderError(text) {
  const div = document.createElement("div");
  div.className = "error";
  div.textContent = "⚠ " + text;
  const box = document.getElementById("messages");
  box.appendChild(div);
  box.scrollTop = box.scrollHeight;
}

function escapeHtml(s) {
  const d = document.createElement("div");
  d.textContent = s;
  return d.innerHTML;
}