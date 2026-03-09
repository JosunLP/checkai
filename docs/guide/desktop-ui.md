# Desktop UI

CheckAI now ships with a dedicated Electron desktop application in `desktop/` in addition to the existing browser UI.

## Goals

The desktop app keeps the full engine workspace available while adding desktop-native workflows:

- **Persistent sessions** for saved backend URLs, launch arguments, and the last active view
- **Native file and folder dialogs** for engine executables, opening books, tablebases, and working directories
- **Local backend launch controls** so the app can start and stop `checkai serve`
- **Inline log inspection** for backend stdout/stderr
- **Dedicated multi-panel layout** with workspace, live engine, engine configuration, logs, and help views
- **Keyboard shortcuts** including a quick-action palette (`⌘/Ctrl + K`)
- **Desktop self-updates** for packaged builds via GitHub Releases
- **Loopback-only embedded live view** so only local backends are rendered inside the Electron shell; non-local targets can still be opened externally

## Technology Stack

| Layer | Technology |
| --- | --- |
| Main process | Electron |
| Renderer | TypeScript + [@bquery/bquery](https://www.npmjs.com/package/@bquery/bquery) reactive signals |
| Bundler | Vite |
| Packaging | electron-builder |

## Build and Run

```bash
cd desktop
corepack enable
npm ci
npm run build
npm run start
```

For local packaging:

```bash
cd desktop
npm run pack   # unpacked app bundle
npm run dist   # installable artifacts
```

## Workflow

1. Open the **Workspace** view and set the backend URL you want to target.
2. Optionally configure a local `checkai` executable, launch arguments, working directory, opening book, and tablebase paths in **Engine**.
3. Use **Start backend** to launch the saved local profile.
4. Switch to **Live** to access the complete CheckAI engine UI inside the desktop shell.
5. Use **Logs** to inspect stdout/stderr from the local backend process.
6. Open **Help** to check for packaged desktop updates, download them, and install them on restart.

## Notes

- The desktop app complements the existing web UI; it does not replace it.
- The embedded live workspace is intentionally limited to loopback URLs (`localhost`, `127.0.0.1`, `::1`). Non-local targets can still be opened in your external browser.
- Saved desktop state is stored in Electron's user data directory.
- Desktop self-updates are available only in packaged builds; development runs keep the update controls visible but report that packaged builds are required.
- Windows release builds use the updater-compatible NSIS installer rather than a portable executable so in-app desktop updates can be applied consistently.
