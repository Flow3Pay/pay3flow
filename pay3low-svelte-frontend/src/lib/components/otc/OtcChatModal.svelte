<script lang="ts">
  import { tick } from "svelte";
  import { locale } from "$lib/i18n";
  import { wsUrl } from "$lib/api";
  import { p2pShortAddress, type P2pDeal } from "$lib/otc/p2p";
  import { identiconDataUri } from "$lib/otc/identicon";
  import type { Session } from "$lib/otc/service";

  export let onClose: () => void = () => {};

  type ChatMessage = { sequence: number; id: string; sender: string; content: string; created_at: string };
  let visible = false;
  let messages: ChatMessage[] = [];
  let deal: P2pDeal | null = null;
  let session: Session | null = null;
  let content = "";
  let connected = false;
  let error = "";
  let socket: WebSocket | null = null;
  let reconnectTimer: ReturnType<typeof setTimeout>;
  let connection = 0;
  let viewport: HTMLDivElement;

  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: selfAvatar = deal?.my_avatar || (deal?.my_ever ? identiconDataUri(deal.my_ever) : "");
  $: otherAvatar = deal?.counterparty_avatar || (deal?.counterparty_ever ? identiconDataUri(deal.counterparty_ever) : "");

  export async function openChat(current: P2pDeal, owner: Session) {
    closeSocket();
    deal = current;
    session = owner;
    messages = [];
    content = "";
    error = "";
    connected = false;
    visible = true;
    await tick();
    connect();
  }

  export function closeChat() {
    closeSocket();
    visible = false;
    onClose();
  }

  function closeSocket() {
    connection += 1;
    clearTimeout(reconnectTimer);
    socket?.close();
    socket = null;
    connected = false;
  }

  function connect() {
    if (!deal || !session || !visible) return;
    const attempt = ++connection;
    error = "";
    let mayReconnect = true;
    let client: WebSocket;
    try {
      client = new WebSocket(wsUrl("/ws/otc/chat"));
    } catch {
      error = say("Chat connection failed.", "Не удалось подключиться к чату.");
      return;
    }
    socket = client;
    client.onopen = () => {
      if (attempt !== connection || !deal || !session) return;
      client.send(JSON.stringify({ type: "authenticate", token: session.token, trade_id: deal.id }));
    };
    client.onmessage = async (event) => {
      if (attempt !== connection) return;
      let payload: { type?: string; messages?: ChatMessage[]; message?: ChatMessage; code?: string };
      try {
        payload = JSON.parse(String(event.data));
      } catch {
        error = say("Received an invalid chat response.", "Получен некорректный ответ чата.");
        return;
      }
      if (payload.type === "history") {
        messages = payload.messages || [];
        connected = true;
        await scrollToLatest();
      } else if (payload.type === "message" && payload.message) {
        if (!messages.some(message => message.id === payload.message!.id)) {
          messages = [...messages, payload.message];
          await scrollToLatest();
        }
      } else if (payload.type === "error") {
        error = payload.code === "trade_not_found"
          ? say("This chat is not available for your account.", "Этот чат недоступен для вашего аккаунта.")
          : say("Chat is temporarily unavailable.", "Чат временно недоступен.");
        if (payload.code === "trade_not_found" || payload.code === "unauthorized") mayReconnect = false;
      }
    };
    client.onclose = () => {
      if (attempt !== connection) return;
      connected = false;
      if (mayReconnect && visible) reconnectTimer = setTimeout(connect, 1200);
    };
    client.onerror = () => {
      if (attempt === connection) error = say("Reconnecting to chat…", "Переподключаемся к чату…");
    };
  }

  async function scrollToLatest() {
    await tick();
    if (viewport) viewport.scrollTop = viewport.scrollHeight;
  }

  function send() {
    const text = content.trim();
    if (!text || text.length > 2000 || !connected || socket?.readyState !== WebSocket.OPEN) return;
    socket.send(JSON.stringify({ type: "send", content: text }));
    content = "";
  }

  function submitOnEnter(event: KeyboardEvent) {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      send();
    }
  }
</script>

<section
  class="chatPanel"
  class:visible
  aria-label={say("P2P chat", "P2P-чат")}
  data-testid="otc-p2p-chat-modal"
>
  {#if visible && deal && session}
    <header class="chatHead">
      <div class="chatTitle">
        <strong>{say("Deal chat", "Чат сделки")}</strong>
        <small>{p2pShortAddress(deal.counterparty_ever || "")}</small>
      </div>
    </header>
    <p class="chatPrivacy">{say("Encrypted at rest. Support can review messages when investigating a report.", "Сообщения зашифрованы при хранении. Поддержка может прочитать их при разборе жалобы.")}</p>
    <div class="chatMessages" bind:this={viewport} aria-live="polite" data-testid="otc-p2p-chat-messages">
      {#if !messages.length && connected}
        <p class="chatEmpty">{say("Start the conversation", "Начните переписку")}</p>
      {/if}
      {#each messages as message (message.id)}
        {@const own = message.sender === session.actor}
        <div class="messageRow" class:own>
          <img class="messageAvatar" src={own ? selfAvatar : otherAvatar} alt="" />
          <div class="messageBubble">
            <small>{own ? say("You", "Вы") : p2pShortAddress(deal.counterparty_ever || "")}</small>
            <p>{message.content}</p>
            <time>{new Date(message.created_at).toLocaleTimeString($locale === "ru" ? "ru-RU" : "en-US", { hour: "2-digit", minute: "2-digit" })}</time>
          </div>
        </div>
      {/each}
      {#if !connected && !error}
        <p class="chatEmpty">{say("Connecting…", "Подключаемся…")}</p>
      {/if}
      {#if error}<p class="chatError" role="status">{error}</p>{/if}
    </div>
    <form class="chatComposer" on:submit|preventDefault={send}>
      <textarea
        bind:value={content}
        maxlength="2000"
        rows="1"
        aria-label={say("Message", "Сообщение")}
        placeholder={say("Write a message…", "Написать сообщение…")}
        disabled={!connected}
        on:keydown={submitOnEnter}
        data-testid="otc-p2p-chat-input"
      ></textarea>
      <button type="submit" class="chatSend" disabled={!connected || !content.trim()} aria-label={say("Send message", "Отправить сообщение")}>
        <svg width="18" height="18" viewBox="0 0 20 20" fill="none" aria-hidden="true"><path d="M10 15V5M5.5 9.5 10 5l4.5 4.5" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>
      </button>
    </form>
  {/if}
</section>

<style>
  .chatPanel { box-sizing:border-box; display:none; min-width:0; height:100%; grid-template-rows:auto auto minmax(0,1fr) auto; overflow:hidden; border:1px solid var(--color-border-strong); border-radius:18px; background:var(--color-paper); color:var(--color-text); }
  .chatPanel.visible { display:grid; }
  .chatHead { display:flex; align-items:center; justify-content:space-between; gap:12px; padding:18px 18px 0; }
  .chatTitle { display:grid; min-width:0; gap:4px; }
  .chatTitle strong { font-size:14px; font-weight:750; }
  .chatTitle small { overflow:hidden; color:var(--color-text-faint); font-family:var(--font-mono); font-size:10px; text-overflow:ellipsis; white-space:nowrap; }
  .chatPrivacy { margin:12px 18px 0; color:var(--color-text-faint); font-size:9px; line-height:1.45; }
  .chatMessages { display:flex; min-height:0; flex-direction:column; gap:10px; overflow-y:auto; padding:14px 14px 10px; overscroll-behavior:contain; }
  .chatEmpty,.chatError { margin:auto 0; color:var(--color-text-faint); font-size:11px; text-align:center; }
  .chatError { color:var(--color-danger); }
  .messageRow { display:flex; max-width:100%; align-items:flex-end; gap:8px; animation:messageIn .24s cubic-bezier(.22, 1, .36, 1) both; }
  .messageRow.own { align-self:flex-end; }
  .messageAvatar { width:20px; height:20px; flex:0 0 20px; border:1px solid var(--color-border); border-radius:6px; background:var(--color-panel); object-fit:cover; }
  .messageBubble { display:grid; max-width:min(270px,calc(100vw - 100px)); gap:4px; padding:9px 11px 7px; border:1px solid var(--color-border); border-radius:13px 13px 13px 4px; background:var(--color-panel); }
  .own .messageBubble { border-color:color-mix(in srgb,var(--color-accent-strong) 35%,var(--color-border)); border-radius:13px 13px 4px 13px; background:var(--otc-buy-soft); }
  .messageBubble small { color:var(--color-text-faint); font-size:9px; }
  .messageBubble p { margin:0; color:var(--color-text); font-size:12px; line-height:1.45; overflow-wrap:anywhere; white-space:pre-wrap; }
  .messageBubble time { justify-self:end; color:var(--color-text-faint); font-size:8px; }
  .chatComposer { display:flex; min-height:58px; align-items:flex-end; gap:8px; padding:9px 12px 12px; border-top:1px solid var(--color-border); }
  .chatComposer textarea { min-width:0; min-height:38px; max-height:100px; flex:1 1 auto; resize:vertical; padding:10px 11px; border:1px solid var(--color-border); border-radius:10px; background:var(--color-paper); color:var(--color-text); font:inherit; font-size:12px; line-height:1.35; }
  .chatComposer textarea:focus-visible,.chatSend:focus-visible { outline:var(--focus-ring-width) solid var(--color-focus); outline-offset:2px; }
  .chatSend { display:grid; width:38px; height:38px; flex:0 0 38px; place-items:center; border:1px solid var(--color-accent-strong); border-radius:10px; background:var(--color-accent); color:#152016; }
  .chatSend:disabled { opacity:.45; cursor:default; }
  @keyframes messageIn { from { opacity:.35; transform:translateY(6px); } }
  @media (prefers-reduced-motion:reduce) { .messageRow { animation:none; } }
</style>
