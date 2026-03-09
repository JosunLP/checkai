import './styles.css';

import { computed, effect, signal } from '@bquery/bquery/reactive';

type DesktopView = 'workspace' | 'live' | 'engine' | 'logs' | 'help';

interface DesktopState {
  backendUrl: string;
  autoStartBackend: boolean;
  backendExecutable: string;
  backendArgs: string;
  backendWorkingDirectory: string;
  openingBookPath: string;
  tablebasePath: string;
  lastView: DesktopView;
}

interface BackendStatusPayload {
  running: boolean;
  pid: number | null;
  command: string | null;
  startedAt: number | null;
  exitCode: number | null;
  lastError: string | null;
}

interface DesktopApi {
  getState(): Promise<DesktopState>;
  saveState(state: DesktopState): Promise<DesktopState>;
  getBackendStatus(): Promise<BackendStatusPayload>;
  getBackendLogs(): Promise<string>;
  startBackend(state: DesktopState): Promise<BackendStatusPayload>;
  stopBackend(): Promise<BackendStatusPayload>;
  pickFile(): Promise<string | null>;
  pickDirectory(): Promise<string | null>;
  openPath(path: string): Promise<void>;
  openExternal(url: string): Promise<void>;
  notify(title: string, body: string): Promise<void>;
  onBackendStatus(callback: (status: BackendStatusPayload) => void): () => void;
  onBackendLogs(callback: (logs: string) => void): () => void;
}

declare global {
  interface Window {
    checkaiDesktop?: DesktopApi;
  }
}

const DEFAULT_STATE: DesktopState = {
  backendUrl: 'http://127.0.0.1:8080',
  autoStartBackend: false,
  backendExecutable: 'checkai',
  backendArgs: 'serve',
  backendWorkingDirectory: '',
  openingBookPath: '',
  tablebasePath: '',
  lastView: 'workspace',
};

const fallbackApi: DesktopApi = {
  async getState() {
    return { ...DEFAULT_STATE };
  },
  async saveState(state) {
    return state;
  },
  async getBackendStatus() {
    return {
      running: false,
      pid: null,
      command: null,
      startedAt: null,
      exitCode: null,
      lastError: 'Electron preload bridge unavailable.',
    };
  },
  async getBackendLogs() {
    return 'Electron preload bridge unavailable. Open this build inside Electron to use native features.';
  },
  async startBackend() {
    return this.getBackendStatus();
  },
  async stopBackend() {
    return this.getBackendStatus();
  },
  async pickFile() {
    return null;
  },
  async pickDirectory() {
    return null;
  },
  async openPath() {},
  async openExternal() {},
  async notify() {},
  onBackendStatus() {
    return () => undefined;
  },
  onBackendLogs() {
    return () => undefined;
  },
};

const desktop = window.checkaiDesktop ?? fallbackApi;

const desktopState = signal<DesktopState>({ ...DEFAULT_STATE });
const currentView = signal<DesktopView>('workspace');
const backendStatus = signal<BackendStatusPayload>({
  running: false,
  pid: null,
  command: null,
  startedAt: null,
  exitCode: null,
  lastError: null,
});
const backendLogs = signal('');
const paletteOpen = signal(false);
const liveReloadToken = signal(Date.now());
const message = signal<string | null>(null);

const liveUrl = computed(() => {
  const base = desktopState.value.backendUrl.trim().replace(/\/+$/, '');
  const token = liveReloadToken.value;
  return base ? `${base}/?desktop=1&t=${token}` : '';
});

function escapeHtml(value: string): string {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');
}

function setMessage(value: string | null): void {
  message.value = value;
  if (!value) return;
  window.setTimeout(() => {
    if (message.value === value) {
      message.value = null;
    }
  }, 3200);
}

function updateState(patch: Partial<DesktopState>): void {
  desktopState.value = { ...desktopState.value, ...patch };
}

async function saveSettings(): Promise<void> {
  const state = { ...desktopState.value, lastView: currentView.value };
  desktopState.value = await desktop.saveState(state);
  setMessage('Desktop settings saved.');
  await desktop.notify('CheckAI Desktop', 'Desktop settings saved.');
}

async function refreshLogs(): Promise<void> {
  backendLogs.value = await desktop.getBackendLogs();
}

async function startBackend(): Promise<void> {
  const state = { ...desktopState.value, lastView: currentView.value };
  backendStatus.value = await desktop.startBackend(state);
  await refreshLogs();
}

async function stopBackend(): Promise<void> {
  backendStatus.value = await desktop.stopBackend();
  await refreshLogs();
}

async function chooseExecutable(): Promise<void> {
  const file = await desktop.pickFile();
  if (file) {
    updateState({ backendExecutable: file });
  }
}

async function chooseWorkingDirectory(): Promise<void> {
  const dir = await desktop.pickDirectory();
  if (dir) {
    updateState({ backendWorkingDirectory: dir });
  }
}

async function chooseOpeningBook(): Promise<void> {
  const file = await desktop.pickFile();
  if (file) {
    updateState({ openingBookPath: file });
  }
}

async function chooseTablebase(): Promise<void> {
  const dir = await desktop.pickDirectory();
  if (dir) {
    updateState({ tablebasePath: dir });
  }
}

function openLiveInBrowser(): void {
  const url = desktopState.value.backendUrl.trim();
  if (!url) return;
  void desktop.openExternal(url);
}

function reloadLiveView(): void {
  liveReloadToken.value = Date.now();
}

function renderCommandPalette(): string {
  if (!paletteOpen.value) return '';

  return `
    <div class="overlay" data-close-palette>
      <div class="palette" role="dialog" aria-modal="true" aria-label="Quick actions">
        <div class="palette-header">
          <h3>Quick actions</h3>
          <button class="ghost-icon-btn" data-close-palette>✕</button>
        </div>
        <div class="palette-actions">
          <button class="palette-action" data-palette-action="start">Start local backend</button>
          <button class="palette-action" data-palette-action="stop">Stop local backend</button>
          <button class="palette-action" data-palette-action="reload">Reload live workspace</button>
          <button class="palette-action" data-palette-action="logs">Open logs view</button>
          <button class="palette-action" data-palette-action="browser">Open engine UI in browser</button>
        </div>
      </div>
    </div>
  `;
}

function renderWorkspaceView(): string {
  return `
    <section class="grid-two">
      <article class="card">
        <div class="card-header">
          <div>
            <h2>Workspace session</h2>
            <p>Persist backend targets, working directories, and native desktop preferences between launches.</p>
          </div>
          <span class="badge ${backendStatus.value.running ? 'badge-success' : 'badge-muted'}">
            ${backendStatus.value.running ? 'Engine running' : 'Engine stopped'}
          </span>
        </div>
        <label class="field">
          <span>Server URL</span>
          <input id="backend-url" value="${escapeHtml(desktopState.value.backendUrl)}" placeholder="http://127.0.0.1:8080" />
        </label>
        <label class="field checkbox-row">
          <input id="auto-start" type="checkbox" ${desktopState.value.autoStartBackend ? 'checked' : ''} />
          <span>Auto-start the local backend when the desktop app launches</span>
        </label>
        <div class="button-row">
          <button class="btn btn-primary" data-action="save">Save workspace</button>
          <button class="btn btn-secondary" data-action="open-browser">Open browser UI</button>
        </div>
      </article>

      <article class="card">
        <div class="card-header">
          <div>
            <h2>Desktop quick actions</h2>
            <p>Lean into desktop workflows with native dialogs, keyboard shortcuts, and persistent state.</p>
          </div>
          <span class="shortcut-pill">⌘/Ctrl + K</span>
        </div>
        <div class="quick-grid">
          <button class="quick-card" data-action="start">
            <strong>Launch backend</strong>
            <span>Start the local <code>checkai serve</code> workflow with your saved engine flags.</span>
          </button>
          <button class="quick-card" data-action="live">
            <strong>Open live workspace</strong>
            <span>Jump into the full engine UI, monitoring, analysis, and archive workspace.</span>
          </button>
          <button class="quick-card" data-action="logs">
            <strong>Inspect logs</strong>
            <span>Review stdout/stderr without leaving the desktop shell.</span>
          </button>
          <button class="quick-card" data-action="open-working-directory">
            <strong>Open working directory</strong>
            <span>Reveal the configured project folder in the system file manager.</span>
          </button>
        </div>
      </article>
    </section>
  `;
}

function renderLiveView(): string {
  if (!desktopState.value.backendUrl.trim()) {
    return `
      <article class="card empty-card">
        <h2>No backend URL configured</h2>
        <p>Save a backend URL in the workspace settings or start a local backend to load the full engine UI.</p>
      </article>
    `;
  }

  return `
    <article class="card live-card">
      <div class="card-header">
        <div>
          <h2>Live engine workspace</h2>
          <p>The complete CheckAI engine UI stays available inside the desktop shell for games, archive replay, analysis, and export workflows.</p>
        </div>
        <div class="button-row">
          <button class="btn btn-secondary" data-action="reload-live">Reload</button>
          <button class="btn btn-secondary" data-action="open-browser">Open in browser</button>
        </div>
      </div>
      <div class="iframe-shell">
        <iframe src="${escapeHtml(liveUrl.value)}" title="CheckAI engine workspace"></iframe>
      </div>
    </article>
  `;
}

function renderEngineView(): string {
  return `
    <section class="grid-two">
      <article class="card">
        <div class="card-header">
          <div>
            <h2>Local backend launch</h2>
            <p>Configure how the Electron shell starts <code>checkai</code> for local runs, live monitoring, and productive desktop sessions.</p>
          </div>
        </div>
        <label class="field">
          <span>Executable</span>
          <div class="input-with-button">
            <input id="backend-executable" value="${escapeHtml(desktopState.value.backendExecutable)}" placeholder="checkai" />
            <button class="btn btn-secondary" data-action="pick-executable">Browse</button>
          </div>
        </label>
        <label class="field">
          <span>Arguments</span>
          <input id="backend-args" value="${escapeHtml(desktopState.value.backendArgs)}" placeholder="serve --analysis-depth 30" />
        </label>
        <label class="field">
          <span>Working directory</span>
          <div class="input-with-button">
            <input id="backend-working-directory" value="${escapeHtml(desktopState.value.backendWorkingDirectory)}" placeholder="/path/to/project" />
            <button class="btn btn-secondary" data-action="pick-working-directory">Browse</button>
          </div>
        </label>
        <div class="button-row">
          <button class="btn btn-primary" data-action="start">Start backend</button>
          <button class="btn btn-secondary" data-action="stop">Stop backend</button>
          <button class="btn btn-secondary" data-action="save">Save</button>
        </div>
      </article>

      <article class="card">
        <div class="card-header">
          <div>
            <h2>Engine assets</h2>
            <p>Wire opening books and tablebases into your saved launch profile using native file and folder pickers.</p>
          </div>
        </div>
        <label class="field">
          <span>Opening book (<code>.bin</code>)</span>
          <div class="input-with-button">
            <input id="opening-book-path" value="${escapeHtml(desktopState.value.openingBookPath)}" placeholder="/path/to/book.bin" />
            <button class="btn btn-secondary" data-action="pick-opening-book">Browse</button>
          </div>
        </label>
        <label class="field">
          <span>Tablebase directory</span>
          <div class="input-with-button">
            <input id="tablebase-path" value="${escapeHtml(desktopState.value.tablebasePath)}" placeholder="/path/to/tablebases" />
            <button class="btn btn-secondary" data-action="pick-tablebase">Browse</button>
          </div>
        </label>
        <div class="callout">
          <strong>Applied automatically:</strong>
          <span>
            When these fields are set, CheckAI Desktop appends <code>--book-path</code> and
            <code>--tablebase-path</code> to the saved backend launch profile unless you already passed them manually.
          </span>
        </div>
      </article>
    </section>
  `;
}

function renderLogsView(): string {
  return `
    <article class="card">
      <div class="card-header">
        <div>
          <h2>Backend logs</h2>
          <p>Tail the local engine process directly inside the desktop app for debugging, monitoring, and failure analysis.</p>
        </div>
        <div class="button-row">
          <button class="btn btn-secondary" data-action="refresh-logs">Refresh</button>
          <button class="btn btn-secondary" data-action="open-working-directory">Open working directory</button>
        </div>
      </div>
      <pre class="log-panel">${escapeHtml(backendLogs.value || 'No backend logs captured yet.')}</pre>
    </article>
  `;
}

function renderHelpView(): string {
  return `
    <section class="grid-two">
      <article class="card">
        <div class="card-header">
          <div>
            <h2>Engine coverage</h2>
            <p>The desktop shell complements the existing web workspace instead of replacing it.</p>
          </div>
        </div>
        <ul class="feature-list">
          <li>Full game creation, move entry, draw/resign actions, and archive replay via the embedded engine UI</li>
          <li>Async analysis workflows, summaries, and result inspection inside the live workspace</li>
          <li>Native desktop file/folder dialogs for engine assets and working directories</li>
          <li>Persistent sessions with saved backend URLs, launch settings, and last-used view</li>
          <li>Inline stdout/stderr log inspection for local backend debugging</li>
        </ul>
      </article>

      <article class="card">
        <div class="card-header">
          <div>
            <h2>Keyboard shortcuts</h2>
            <p>Desktop-friendly interactions for faster navigation.</p>
          </div>
        </div>
        <ul class="feature-list">
          <li><strong>⌘/Ctrl + K</strong> — Open the command palette</li>
          <li><strong>⌘/Ctrl + 1…5</strong> — Switch between workspace, live, engine, logs, and help views</li>
          <li><strong>Escape</strong> — Close the command palette</li>
        </ul>
      </article>
    </section>
  `;
}

function renderMainContent(): string {
  switch (currentView.value) {
    case 'workspace':
      return renderWorkspaceView();
    case 'live':
      return renderLiveView();
    case 'engine':
      return renderEngineView();
    case 'logs':
      return renderLogsView();
    case 'help':
      return renderHelpView();
  }
}

function renderApp(): string {
  return `
    <div class="shell">
      <aside class="sidebar">
        <div class="brand">
          <span class="brand-icon">♔</span>
          <div>
            <strong>CheckAI Desktop</strong>
            <p>Electron + bQuery workspace</p>
          </div>
        </div>
        <nav class="sidebar-nav">
          ${([
            ['workspace', 'Workspace'],
            ['live', 'Live'],
            ['engine', 'Engine'],
            ['logs', 'Logs'],
            ['help', 'Help'],
          ] as Array<[DesktopView, string]>)
            .map(
              ([view, label]) => `
                <button class="sidebar-link ${currentView.value === view ? 'active' : ''}" data-view="${view}">
                  ${label}
                </button>
              `,
            )
            .join('')}
        </nav>
        <div class="sidebar-footer">
          <span class="status-dot ${backendStatus.value.running ? 'online' : 'offline'}"></span>
          <span>${backendStatus.value.running ? 'Local engine online' : 'Local engine idle'}</span>
        </div>
      </aside>

      <main class="content">
        <header class="topbar">
          <div>
            <h1>Dedicated desktop UI for productive engine workflows</h1>
            <p>${escapeHtml(desktopState.value.backendUrl)}</p>
          </div>
          <div class="topbar-actions">
            <div class="status-card">
              <span class="status-title">Backend</span>
              <strong>${backendStatus.value.running ? 'Running' : 'Stopped'}</strong>
              <span class="status-subtle">${escapeHtml(backendStatus.value.command ?? 'Waiting for launch')}</span>
            </div>
            <button class="btn btn-secondary" data-action="toggle-palette">Quick actions</button>
            <button class="btn btn-primary" data-action="start">Start</button>
          </div>
        </header>

        ${
          message.value
            ? `<div class="flash-message">${escapeHtml(message.value)}</div>`
            : ''
        }
        ${
          backendStatus.value.lastError
            ? `<div class="error-banner">${escapeHtml(backendStatus.value.lastError)}</div>`
            : ''
        }

        ${renderMainContent()}
      </main>
    </div>
    ${renderCommandPalette()}
  `;
}

function bindEvents(root: HTMLElement): void {
  root.querySelectorAll<HTMLElement>('[data-view]').forEach((button) => {
    button.addEventListener('click', () => {
      currentView.value = button.dataset.view as DesktopView;
      updateState({ lastView: currentView.value });
      void saveSettings();
    });
  });

  root.querySelector<HTMLElement>('[data-action="toggle-palette"]')?.addEventListener('click', () => {
    paletteOpen.value = true;
  });

  root.querySelectorAll<HTMLElement>('[data-close-palette]').forEach((button) => {
    button.addEventListener('click', () => {
      paletteOpen.value = false;
    });
  });

  root.querySelectorAll<HTMLElement>('[data-palette-action]').forEach((button) => {
    button.addEventListener('click', async () => {
      const action = button.dataset.paletteAction;
      paletteOpen.value = false;
      if (action === 'start') await startBackend();
      if (action === 'stop') await stopBackend();
      if (action === 'reload') reloadLiveView();
      if (action === 'logs') currentView.value = 'logs';
      if (action === 'browser') openLiveInBrowser();
    });
  });

  root.querySelector<HTMLElement>('[data-action="save"]')?.addEventListener('click', () => {
    void saveSettings();
  });
  root.querySelectorAll<HTMLElement>('[data-action="start"]').forEach((button) => {
    button.addEventListener('click', () => void startBackend());
  });
  root.querySelectorAll<HTMLElement>('[data-action="stop"]').forEach((button) => {
    button.addEventListener('click', () => void stopBackend());
  });
  root.querySelector<HTMLElement>('[data-action="open-browser"]')?.addEventListener('click', openLiveInBrowser);
  root.querySelector<HTMLElement>('[data-action="reload-live"]')?.addEventListener('click', reloadLiveView);
  root.querySelector<HTMLElement>('[data-action="refresh-logs"]')?.addEventListener('click', () => void refreshLogs());
  root.querySelector<HTMLElement>('[data-action="pick-executable"]')?.addEventListener('click', () => void chooseExecutable());
  root
    .querySelector<HTMLElement>('[data-action="pick-working-directory"]')
    ?.addEventListener('click', () => void chooseWorkingDirectory());
  root
    .querySelector<HTMLElement>('[data-action="pick-opening-book"]')
    ?.addEventListener('click', () => void chooseOpeningBook());
  root
    .querySelector<HTMLElement>('[data-action="pick-tablebase"]')
    ?.addEventListener('click', () => void chooseTablebase());
  root.querySelector<HTMLElement>('[data-action="live"]')?.addEventListener('click', () => {
    currentView.value = 'live';
  });
  root.querySelector<HTMLElement>('[data-action="logs"]')?.addEventListener('click', () => {
    currentView.value = 'logs';
  });
  root.querySelector<HTMLElement>('[data-action="open-working-directory"]')?.addEventListener('click', () => {
    const path = desktopState.value.backendWorkingDirectory.trim();
    if (path) {
      void desktop.openPath(path);
    }
  });

  root.querySelector<HTMLInputElement>('#backend-url')?.addEventListener('input', (event) => {
    const target = event.currentTarget as HTMLInputElement;
    updateState({ backendUrl: target.value });
  });
  root.querySelector<HTMLInputElement>('#auto-start')?.addEventListener('change', (event) => {
    const target = event.currentTarget as HTMLInputElement;
    updateState({ autoStartBackend: target.checked });
  });
  root.querySelector<HTMLInputElement>('#backend-executable')?.addEventListener('input', (event) => {
    const target = event.currentTarget as HTMLInputElement;
    updateState({ backendExecutable: target.value });
  });
  root.querySelector<HTMLInputElement>('#backend-args')?.addEventListener('input', (event) => {
    const target = event.currentTarget as HTMLInputElement;
    updateState({ backendArgs: target.value });
  });
  root.querySelector<HTMLInputElement>('#backend-working-directory')?.addEventListener('input', (event) => {
    const target = event.currentTarget as HTMLInputElement;
    updateState({ backendWorkingDirectory: target.value });
  });
  root.querySelector<HTMLInputElement>('#opening-book-path')?.addEventListener('input', (event) => {
    const target = event.currentTarget as HTMLInputElement;
    updateState({ openingBookPath: target.value });
  });
  root.querySelector<HTMLInputElement>('#tablebase-path')?.addEventListener('input', (event) => {
    const target = event.currentTarget as HTMLInputElement;
    updateState({ tablebasePath: target.value });
  });
}

function registerKeyboardShortcuts(): void {
  window.addEventListener('keydown', (event) => {
    const shortcut = event.metaKey || event.ctrlKey;
    if (shortcut && event.key.toLowerCase() === 'k') {
      event.preventDefault();
      paletteOpen.value = true;
      return;
    }

    if (shortcut && /^[1-5]$/.test(event.key)) {
      event.preventDefault();
      const order: DesktopView[] = ['workspace', 'live', 'engine', 'logs', 'help'];
      currentView.value = order[Number(event.key) - 1] ?? 'workspace';
      return;
    }

    if (event.key === 'Escape') {
      paletteOpen.value = false;
    }
  });
}

async function init(): Promise<void> {
  desktopState.value = await desktop.getState();
  currentView.value = desktopState.value.lastView;
  backendStatus.value = await desktop.getBackendStatus();
  backendLogs.value = await desktop.getBackendLogs();

  desktop.onBackendStatus((status) => {
    backendStatus.value = status;
  });
  desktop.onBackendLogs((logs) => {
    backendLogs.value = logs;
  });

  registerKeyboardShortcuts();
}

const appRoot = document.getElementById('app');
if (!appRoot) {
  throw new Error('Missing #app root element');
}

effect(() => {
  appRoot.innerHTML = renderApp();
  bindEvents(appRoot);
});

void init();
