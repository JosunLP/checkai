// ============================================================================
// CheckAI Web UI — WebSocket Manager
// ============================================================================

import { store } from './store';
import type { WsMessage, WsPayload } from './types';

const WS_URL = `${window.location.protocol === 'https:' ? 'wss' : 'ws'}://${window.location.host}/ws`;

let ws: WebSocket | null = null;
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
let reconnectDelay = 1000; // Start at 1s, exponential backoff up to 30s
const RECONNECT_MAX_DELAY = 30_000;

type WsEventHandler = (msg: WsMessage) => void;
let onMessage: WsEventHandler | null = null;

/** Registers the handler for incoming WebSocket events. */
export function onWsMessage(handler: WsEventHandler): void {
  onMessage = handler;
}

/** Opens (or re-opens) the WebSocket connection. */
export function connectWebSocket(): void {
  if (
    ws &&
    (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)
  )
    return;

  try {
    ws = new WebSocket(WS_URL);
  } catch {
    scheduleReconnect();
    return;
  }

  ws.onopen = () => {
    store.wsConnected.value = true;
    reconnectDelay = 1000; // Reset backoff on successful connection
    if (reconnectTimer) {
      clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
    const gid = store.currentGameId.value;
    if (gid) wsSubscribe(gid);
  };

  ws.onmessage = (event: MessageEvent) => {
    try {
      const msg: WsMessage = JSON.parse(event.data as string);
      onMessage?.(msg);
    } catch {
      /* ignore malformed messages */
    }
  };

  ws.onclose = () => {
    store.wsConnected.value = false;
    scheduleReconnect();
  };

  ws.onerror = () => {
    store.wsConnected.value = false;
  };
}

function scheduleReconnect(): void {
  if (reconnectTimer) return;
  reconnectTimer = setTimeout(() => {
    reconnectTimer = null;
    connectWebSocket();
  }, reconnectDelay);
  // Exponential backoff with cap
  reconnectDelay = Math.min(reconnectDelay * 2, RECONNECT_MAX_DELAY);
}

function wsSend(payload: WsPayload): void {
  if (ws?.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify(payload));
  }
}

/** Subscribe to real-time updates for a game. */
export function wsSubscribe(gameId: string): void {
  wsSend({ action: 'subscribe', game_id: gameId });
}

/** Unsubscribe from a game's real-time updates. */
export function wsUnsubscribe(gameId: string): void {
  wsSend({ action: 'unsubscribe', game_id: gameId });
}
