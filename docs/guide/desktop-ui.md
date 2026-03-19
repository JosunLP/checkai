# Desktop UI

CheckAI now ships with a dedicated Electron desktop application in `desktop/` in addition to the existing browser UI.

## Goals

The desktop app keeps the full engine workspace available while adding desktop-native workflows:

- **Persistent sessions** for saved backend URLs, launch arguments, and the last active view
- **Local backend launch controls** so the app can start and stop `checkai serve`
- **Inline log inspection** for backend stdout/stderr
- **Dedicated multi-panel layout** with dashboard, game, board, analysis, archive, engine, log, and settings views
- **Keyboard shortcuts** including a quick-action palette (`⌘/Ctrl + K`)

## Technology Stack

| Layer | Technology |
| --- | --- |
| Main process | Electron |
| Renderer | Svelte + TypeScript |
| Styling | SCSS + Tailwind CSS v4 |
| Bundler | Vite |
| Packaging | electron-builder |

## Build and Run

```bash
cd desktop
bun install --frozen-lockfile
bun run build
bun run start
```

For local packaging:

```bash
cd desktop
bun run pack   # unpacked app bundle
bun run dist   # installable artifacts
```

## Workflow

1. Open the **Engine** view and set the backend URL you want to target.
2. Optionally configure a local `checkai` executable, launch arguments, working directory, opening book, and tablebase paths in **Engine**.
3. Use **Start backend** to launch the saved local profile.
4. Use **Dashboard**, **Games**, **Board**, **Analysis**, and **Archive** to move through the desktop workspace views.
5. Use **Logs** to inspect stdout/stderr from the local backend process.
6. Open **Settings** to adjust theme, compact mode, notifications, developer mode, and board orientation.

## Notes

- The desktop app complements the existing web UI; it does not replace it.
- Saved desktop state is stored in Electron's user data directory.
- Release automation publishes updater-compatible desktop artifacts alongside native installers for each platform (`.deb` on Linux, `.dmg` on macOS, `.msi` on Windows).
- Windows release builds keep the updater-compatible NSIS installer in addition to the MSI package so in-app desktop updates can still be applied consistently.
