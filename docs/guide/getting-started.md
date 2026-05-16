# Getting Started

## System Requirements

| Requirement              | Minimum                                            |
| ------------------------ | -------------------------------------------------- |
| OS                       | Linux (glibc 2.31+), macOS 12+, Windows 10+        |
| CPU                      | Any x86-64 or ARM64                                |
| RAM                      | 128 MB (+ transposition table size, default 64 MB) |
| Disk                     | ~20 MB for the binary                              |
| Rust (build from source) | 1.85.0+ (edition 2024)                             |
| Bun (web UI build)       | 1.3.10+                                            |

## Installation

### Pre-built Binaries (Recommended)

Pin the release you want and verify the downloaded binary against the published
SHA-256 checksums before installing it. The examples below use `v0.7.0`; check
the [Releases](https://github.com/JosunLP/checkai/releases) page and replace it
with the current or desired release tag.

::: code-group

```bash [Linux / macOS]
CHECKAI_VERSION=v0.7.0
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
[ "$OS" = "darwin" ] || OS="linux"
ARCH="$(uname -m)"
case "$ARCH" in x86_64|amd64) ARCH=x86_64 ;; arm64|aarch64) ARCH=aarch64 ;; esac
ASSET="checkai-${OS}-${ARCH}"
BASE_URL="https://github.com/JosunLP/checkai/releases/download/${CHECKAI_VERSION}"

curl -fSLO "${BASE_URL}/${ASSET}"
curl -fSLO "${BASE_URL}/checksums-sha256.txt"
if command -v sha256sum >/dev/null 2>&1; then
  grep "  ${ASSET}$" checksums-sha256.txt | sha256sum -c -
elif command -v shasum >/dev/null 2>&1; then
  grep "  ${ASSET}$" checksums-sha256.txt | shasum -a 256 -c -
else
  echo "Error: Neither sha256sum nor shasum found. On Linux, install coreutils; on macOS, shasum should be pre-installed." >&2
  exit 1
fi
chmod +x "${ASSET}"
sudo install -m 0755 "${ASSET}" /usr/local/bin/checkai
```

```powershell [Windows (PowerShell)]
$Version = "v0.7.0"
$Asset = "checkai-windows-x86_64.exe"
$BaseUrl = "https://github.com/JosunLP/checkai/releases/download/$Version"
Invoke-WebRequest "$BaseUrl/$Asset" -OutFile $Asset
Invoke-WebRequest "$BaseUrl/checksums-sha256.txt" -OutFile checksums-sha256.txt
$Expected = ((Select-String .\checksums-sha256.txt -Pattern "  $([regex]::Escape($Asset))$").Line -split "\s+")[0].ToLowerInvariant()
$Actual = (Get-FileHash ".\$Asset" -Algorithm SHA256).Hash.ToLowerInvariant()
if ($Actual -ne $Expected) { throw "Checksum verification failed for $Asset" }
New-Item -ItemType Directory "$env:LOCALAPPDATA\checkai" -Force | Out-Null
Move-Item -Force ".\$Asset" "$env:LOCALAPPDATA\checkai\checkai.exe"
```

:::

Windows ARM64 users should use the `checkai-windows-x86_64.exe` asset under
Windows' x86-64 emulation until a native ARM64 CLI binary is published.

#### Installer shortcut

The installer can still detect your operating system, architecture, and latest
release automatically:

::: code-group

```bash [Linux / macOS]
curl -fsSL https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/install.sh | sh
```

```powershell [Windows (PowerShell)]
irm https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/install.sh | iex
```

:::

::: warning
The installer shortcut executes the current `main` branch script immediately and
is less secure than verifying a pinned release asset first. Only use it if you
trust the source and accept that trade-off. If you want a manual review step,
open or download the same URL first, read through the script carefully, and only
then run it yourself. You can also inspect the matching script in the
repository's `scripts/` directory before executing it.
:::

### Build from Source

You need [Rust](https://www.rust-lang.org/tools/install) and [Bun](https://bun.sh/) installed.

```bash
git clone https://github.com/JosunLP/checkai.git
cd checkai

# Build the embedded web UI first
cd web
bun install --frozen-lockfile
bun run build
cd ..

# Then build the Rust server/CLI
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
curl -fsSL https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/uninstall.sh | sh
```

```powershell [Windows (PowerShell)]
irm https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/uninstall.sh | iex
```

:::

::: warning
The one-line commands in the Uninstall section execute a remote script immediately.
Only use them if you trust the source. If you want a manual review step, open or download the same URL first, read through the script carefully, and only then run it yourself. You can also inspect the matching script in the repository's `scripts/` directory before executing it.
:::

## Next Steps

- [CLI Commands](./cli) — All available commands and flags
- [REST API Reference](../api/rest) — Complete endpoint documentation
- [Agent Protocol](../agent/overview) — How AI agents interact with CheckAI
