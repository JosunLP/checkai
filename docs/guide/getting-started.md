# Getting Started

## System Requirements

| Requirement              | Minimum                                            |
| ------------------------ | -------------------------------------------------- |
| OS                       | Linux (glibc 2.31+), macOS 12+, Windows 10+        |
| CPU                      | Any x86-64 or ARM64                                |
| RAM                      | 128 MB (+ transposition table size, default 64 MB) |
| Disk                     | ~20 MB for the binary                              |
| Rust (build from source) | 1.85.0+ (edition 2024)                             |
| Node.js (web UI dev)     | 18+                                                |

## Installation

### Pre-built Binaries (Recommended)

Download the install script for a **pinned release**, verify its checksum, then run it:

::: code-group

```bash [Linux / macOS]
# 1. Set the version you want to install
VERSION="0.4.0"

# 2. Download the install script from the pinned release tag
curl -fsSL -o install.sh \
  "https://raw.githubusercontent.com/JosunLP/checkai/v${VERSION}/scripts/install.sh"

# 3. Download the checksum file and verify
curl -fsSL -o install.sh.sha256 \
  "https://github.com/JosunLP/checkai/releases/download/v${VERSION}/install.sh.sha256"
sha256sum -c install.sh.sha256

# 4. Inspect the script before running it
less install.sh

# 5. Execute
sh install.sh
```

```powershell [Windows]
# 1. Set the version you want to install
$Version = "0.4.0"

# 2. Download the install script from the pinned release tag
Invoke-WebRequest `
  -Uri "https://raw.githubusercontent.com/JosunLP/checkai/v$Version/scripts/install.ps1" `
  -OutFile install.ps1

# 3. Download the checksum file and verify
Invoke-WebRequest `
  -Uri "https://github.com/JosunLP/checkai/releases/download/v$Version/install.ps1.sha256" `
  -OutFile install.ps1.sha256
$expected = (Get-Content install.ps1.sha256).Split(' ')[0]
$actual   = (Get-FileHash install.ps1 -Algorithm SHA256).Hash.ToLower()
if ($actual -ne $expected) { throw "Checksum mismatch! Aborting." }

# 4. Inspect the script before running it
Get-Content install.ps1 | Out-Host -Paging

# 5. Execute
.\install.ps1
```

:::

> [!WARNING]
> **Never** pipe remote scripts directly into a shell (`curl | sh`, `irm | iex`).
> Always download, verify, and inspect scripts before executing them.

### Build from Source

You need [Rust](https://www.rust-lang.org/tools/install) installed (edition 2024).

```bash
git clone https://github.com/JosunLP/checkai.git
cd checkai
cargo build --release
```

The binary will be at `target/release/checkai` (or `checkai.exe` on Windows).

### Docker

```bash
docker compose up -d
```

See the [Docker guide](./docker) for full details.

## Quick Start

### Start the API Server

```bash
# Default: http://0.0.0.0:8080
checkai serve

# Custom port
checkai serve --port 3000
```

Swagger UI is automatically available at `http://localhost:8080/swagger-ui/`.

### Create and Play a Game via API

```bash
# 1. Create a new game
curl -X POST http://localhost:8080/api/games

# 2. Get game state
curl http://localhost:8080/api/games/{game_id}

# 3. Submit a move (1. e4)
curl -X POST http://localhost:8080/api/games/{game_id}/move \
  -H "Content-Type: application/json" \
  -d '{"from": "e2", "to": "e4", "promotion": null}'

# 4. Get legal moves
curl http://localhost:8080/api/games/{game_id}/moves
```

### Play in the Terminal

```bash
checkai play
```

This starts an interactive two-player game with a colored board display. Type `help` for available commands.

## Uninstall

::: code-group

```bash [Linux / macOS]
# 1. Set the version that was installed
VERSION="0.4.0"

# 2. Download the uninstall script from the pinned release tag
curl -fsSL -o uninstall.sh \
  "https://raw.githubusercontent.com/JosunLP/checkai/v${VERSION}/scripts/uninstall.sh"

# 3. Download the checksum file and verify
curl -fsSL -o uninstall.sh.sha256 \
  "https://github.com/JosunLP/checkai/releases/download/v${VERSION}/uninstall.sh.sha256"
sha256sum -c uninstall.sh.sha256

# 4. Inspect, then execute
less uninstall.sh
sh uninstall.sh
```

```powershell [Windows]
# 1. Set the version that was installed
$Version = "0.4.0"

# 2. Download the uninstall script from the pinned release tag
Invoke-WebRequest `
  -Uri "https://raw.githubusercontent.com/JosunLP/checkai/v$Version/scripts/uninstall.ps1" `
  -OutFile uninstall.ps1

# 3. Download the checksum file and verify
Invoke-WebRequest `
  -Uri "https://github.com/JosunLP/checkai/releases/download/v$Version/uninstall.ps1.sha256" `
  -OutFile uninstall.ps1.sha256
$expected = (Get-Content uninstall.ps1.sha256).Split(' ')[0]
$actual   = (Get-FileHash uninstall.ps1 -Algorithm SHA256).Hash.ToLower()
if ($actual -ne $expected) { throw "Checksum mismatch! Aborting." }

# 4. Inspect, then execute
Get-Content uninstall.ps1 | Out-Host -Paging
.\uninstall.ps1
```

:::

## Next Steps

- [CLI Commands](./cli) — All available commands and flags
- [REST API Reference](../api/rest) — Complete endpoint documentation
- [Agent Protocol](../agent/overview) — How AI agents interact with CheckAI
