# Special Actions

In certain situations, the agent returns a special JSON object instead of a move.

## Draw Claim

When a draw condition is met:

```json
{ "action": "claim_draw", "reason": "threefold_repetition" }
```

```json
{ "action": "claim_draw", "reason": "fifty_move_rule" }
```

## Draw Offer

Propose a draw to the opponent:

```json
{ "action": "offer_draw" }
```

The draw offer persists after the offerer makes their move. The opponent can then accept or decline (by making a move).

## Accept Draw

Accept a pending draw offer:

```json
{ "action": "accept_draw" }
```

## Resignation

Forfeit the game:

```json
{ "action": "resign" }
```

## Rules

- An agent **must** use `claim_draw` (not resign) when a draw condition is met and the position is clearly lost
- Draw offers persist until the opponent's next action (accept or decline by moving)
- A player cannot accept their own draw offer
- Resignation immediately ends the game

## Game End Summary

| Event                     | Result                  | Trigger          |
| ------------------------- | ----------------------- | ---------------- |
| Checkmate                 | Win for the attacker    | Automatic        |
| Stalemate                 | Draw                    | Automatic        |
| Threefold repetition (3×) | Draw (claimable)        | Agent output     |
| Fivefold repetition (5×)  | Draw                    | Automatic        |
| 50-move rule (≥ 100 HM)   | Draw (claimable)        | Agent output     |
| 75-move rule (≥ 150 HM)   | Draw                    | Automatic        |
| Dead position             | Draw                    | Automatic        |
| Agreement                 | Draw                    | Both sides agree |
| Resignation               | Loss for resigning side | Agent output     |
