# WebSocket API

CheckAI provides a full-featured WebSocket API at `/ws` that mirrors every REST endpoint and adds real-time event broadcasting.

## Connection

```bash
ws://localhost:8080/ws
```

Or over TLS:

```bash
wss://your-host/ws
```

## Message Format

All client-to-server messages are JSON objects with an `"action"` field. Server responses include a `"type"` field to distinguish responses from events.

### Client → Server

```json
{
  "action": "create_game",
  "request_id": "abc123"
}
```

The `request_id` field is optional but recommended for correlating responses.

## Available Actions

### Game Management

| Action        | Extra Fields | Description       |
| ------------- | ------------ | ----------------- |
| `create_game` | —            | Create a new game |
| `list_games`  | —            | List all games    |
| `get_game`    | `game_id`    | Get game state    |
| `delete_game` | `game_id`    | Delete a game     |

### Gameplay

| Action            | Extra Fields                          | Description             |
| ----------------- | ------------------------------------- | ----------------------- |
| `submit_move`     | `game_id`, `from`, `to`, `promotion?` | Submit a move           |
| `submit_action`   | `game_id`, `action_type`, `reason?`   | Submit a special action |
| `get_legal_moves` | `game_id`                             | Get legal moves         |
| `get_board`       | `game_id`                             | Get ASCII board         |

### Subscriptions

| Action        | Extra Fields | Description                              |
| ------------- | ------------ | ---------------------------------------- |
| `subscribe`   | `game_id`    | Subscribe to real-time events for a game |
| `unsubscribe` | `game_id`    | Unsubscribe from a game                  |

### Archive

| Action            | Extra Fields              | Description             |
| ----------------- | ------------------------- | ----------------------- |
| `list_archived`   | —                         | List all archived games |
| `get_archived`    | `game_id`                 | Get an archived game    |
| `replay_archived` | `game_id`, `move_number?` | Replay an archived game |

### Storage

| Action              | Extra Fields | Description            |
| ------------------- | ------------ | ---------------------- |
| `get_storage_stats` | —            | Get storage statistics |

## Server → Client Messages

### Response

Sent in reply to a client action:

```json
{
  "type": "response",
  "action": "submit_move",
  "request_id": "abc123",
  "success": true,
  "data": { ... }
}
```

### Event

Pushed to all subscribers of a game when something changes:

```json
{
  "type": "event",
  "event": "game_updated",
  "game_id": "550e8400-...",
  "data": { ... }
}
```

Event types include:

| Event          | Description                      |
| -------------- | -------------------------------- |
| `game_updated` | A move was made or state changed |
| `game_deleted` | A game was deleted               |

## Example (JavaScript)

```javascript
const ws = new WebSocket("ws://localhost:8080/ws");

ws.onopen = () => {
  // Create a game
  ws.send(JSON.stringify({
    action: "create_game",
    request_id: "1"
  }));
};

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);

  if (msg.type === "response" && msg.action === "create_game") {
    const gameId = msg.data.game_id;

    // Subscribe to real-time events
    ws.send(JSON.stringify({
      action: "subscribe",
      game_id: gameId
    }));

    // Make a move
    ws.send(JSON.stringify({
      action: "submit_move",
      game_id: gameId,
      from: "e2",
      to: "e4",
      request_id: "2"
    }));
  }

  if (msg.type === "event") {
    console.log("Game event:", msg.event, msg.data);
  }
};

ws.onerror = (err) => console.error("WebSocket error:", err);
ws.onclose = () => console.log("WebSocket closed");
```

## Example (Python)

```python
import json
import websocket

def on_message(ws, message):
    msg = json.loads(message)
    if msg["type"] == "response" and msg["action"] == "create_game":
        game_id = msg["data"]["game_id"]
        ws.send(json.dumps({
            "action": "subscribe",
            "game_id": game_id
        }))
        ws.send(json.dumps({
            "action": "submit_move",
            "game_id": game_id,
            "from": "e2",
            "to": "e4"
        }))
    elif msg["type"] == "event":
        print(f"Event: {msg['event']}", msg["data"])

ws = websocket.WebSocketApp(
    "ws://localhost:8080/ws",
    on_message=on_message,
    on_open=lambda ws: ws.send(json.dumps({
        "action": "create_game",
        "request_id": "1"
    }))
)
ws.run_forever()
```
