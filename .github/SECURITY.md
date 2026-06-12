# Security Policy

## Supported versions

CheckAI follows a rolling-release model. Only the **latest released version**
receives security updates. Please make sure you can reproduce any issue on the
most recent release (or `main`) before reporting it.

| Version        | Supported |
| -------------- | --------- |
| Latest release | ✅         |
| Older releases | ❌         |

## Reporting a vulnerability

**Please do not report security vulnerabilities through public GitHub issues,
pull requests, or discussions.**

Instead, report them privately through GitHub Security Advisories:

➡️ **<https://github.com/JosunLP/checkai/security/advisories/new>**

Please include as much of the following as possible:

- The affected component (CLI, REST API, WebSocket API, engine, web/desktop UI,
  WASM/npm package).
- The CheckAI version (`checkai version`) and your operating system.
- A clear description of the vulnerability and its potential impact.
- Step-by-step reproduction instructions, including any required FEN strings,
  requests, or payloads.

We will acknowledge your report as quickly as we can, keep you informed about
the progress of a fix, and credit you in the release notes if you wish.

## Disclosure policy

We follow a coordinated-disclosure process: we ask that you give us a
reasonable opportunity to investigate and release a fix before any public
disclosure.

## Deployment security notes

CheckAI is primarily designed as a developer tool and an integration target for
AI agents. Keep the following in mind when deploying it:

- **Binds to `0.0.0.0` by default.** `checkai serve` listens on all interfaces.
  Bind to `127.0.0.1` (via `--host`) or place it behind a firewall/reverse
  proxy when you do not want it reachable from the network.
- **Permissive CORS.** The REST/WebSocket API enables cross-origin access for
  all origins so browser-based and agent clients can connect easily. Run it on
  a trusted network or add an authenticating proxy if you need access control.
- **No built-in authentication.** There is no user authentication layer; any
  client that can reach the port can create and manipulate games.
- **Self-update.** `checkai update` downloads release binaries from GitHub over
  HTTPS and replaces the running executable. Only run it where that is
  acceptable.

These are intentional design choices for a local/trusted-network developer
tool, not vulnerabilities — but they are important to understand before
exposing CheckAI to untrusted networks.
