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

interface UpdateStatusPayload {
  supported: boolean;
  currentVersion: string;
  state:
    | 'idle'
    | 'unsupported'
    | 'checking'
    | 'available'
    | 'downloading'
    | 'downloaded'
    | 'up-to-date'
    | 'error';
  availableVersion: string | null;
  percent: number | null;
  transferredBytes: number | null;
  totalBytes: number | null;
  message: string | null;
}

interface DesktopApi {
  getState(): Promise<DesktopState>;
  saveState(state: DesktopState): Promise<DesktopState>;
  getBackendStatus(): Promise<BackendStatusPayload>;
  getBackendLogs(): Promise<string>;
  getUpdateStatus(): Promise<UpdateStatusPayload>;
  startBackend(state: DesktopState): Promise<BackendStatusPayload>;
  stopBackend(): Promise<BackendStatusPayload>;
  checkForUpdates(): Promise<UpdateStatusPayload>;
  downloadUpdate(): Promise<UpdateStatusPayload>;
  installUpdate(): Promise<void>;
  pickFile(): Promise<string | null>;
  pickDirectory(): Promise<string | null>;
  openPath(path: string): Promise<void>;
  openExternal(url: string): Promise<void>;
  notify(title: string, body: string): Promise<void>;
  onBackendStatus(callback: (status: BackendStatusPayload) => void): () => void;
  onBackendLogs(callback: (logs: string) => void): () => void;
  onUpdateStatus(callback: (status: UpdateStatusPayload) => void): () => void;
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
  async getUpdateStatus() {
    return {
      supported: false,
      currentVersion: 'dev',
      state: 'unsupported',
      availableVersion: null,
      percent: null,
      transferredBytes: null,
      totalBytes: null,
      message: 'Desktop updates are available in packaged builds.',
    };
  },
  async checkForUpdates() {
    return this.getUpdateStatus();
  },
  async downloadUpdate() {
    return this.getUpdateStatus();
  },
  async installUpdate() {},
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
  onUpdateStatus() {
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
const updateStatus = signal<UpdateStatusPayload>({
  supported: false,
  currentVersion: 'dev',
  state: 'unsupported',
  availableVersion: null,
  percent: null,
  transferredBytes: null,
  totalBytes: null,
  message: 'Desktop updates are available in packaged builds.',
});
const paletteOpen = signal(false);
const liveReloadToken = signal(Date.now());
const message = signal<string | null>(null);

const liveUrl = computed(() => {
  const base = desktopState.value.backendUrl.trim().replace(/\/+$/, '');
  const token = liveReloadToken.value;
  return base ? `${base}/?desktop=1&t=${token}` : '';
});

const canEmbedLiveView = computed(() => {
  try {
    const url = new URL(desktopState.value.backendUrl);
    return (
      (url.protocol === 'http:' || url.protocol === 'https:') &&
      ['127.0.0.1', 'localhost', '::1'].includes(url.hostname)
    );
  } catch {
    return false;
  }
});

function escapeHtml(value: string): string {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');
}

function formatBytes(value: number | null): string {
  if (value === null || !Number.isFinite(value)) return '—';
  if (value < 1024) return `${value} B`;

  const units = ['KB', 'MB', 'GB', 'TB'];
  let size = value / 1024;
  let unitIndex = 0;

  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }

  return `${size.toFixed(size >= 10 ? 0 : 1)} ${units[unitIndex]}`;
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
  await persistState(true);
}

async function persistState(announce: boolean): Promise<void> {
  const state = { ...desktopState.value, lastView: currentView.value };
  desktopState.value = await desktop.saveState(state);
  if (announce) {
    setMessage('Desktop settings saved.');
    await desktop.notify('CheckAI Desktop', 'Desktop settings saved.');
  }
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

async function checkForDesktopUpdates(): Promise<void> {
  updateStatus.value = await desktop.checkForUpdates();
}

async function downloadDesktopUpdate(): Promise<void> {
  updateStatus.value = await desktop.downloadUpdate();
}

async function installDesktopUpdate(): Promise<void> {
  await desktop.installUpdate();
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
          <button class="palette-action" data-palette-action="check-updates">Check desktop updates</button>
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

  if (!canEmbedLiveView.value) {
    return `
      <article class="card empty-card">
        <div>
          <h2>Embedded live view is limited to local backends</h2>
          <p>
            For safety, the Electron workspace only embeds loopback URLs. You can still open the configured
            backend in your external browser.
          </p>
          <div class="button-row center-row">
            <button class="btn btn-secondary" data-action="open-browser">Open in browser</button>
            <button class="btn btn-secondary" data-action="reload-live">Reload URL</button>
          </div>
        </div>
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
        <iframe
          src="${escapeHtml(liveUrl.value)}"
          title="CheckAI engine workspace"
          sandbox="allow-downloads allow-forms allow-popups allow-same-origin allow-scripts"
        ></iframe>
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

      <article class="card">
        <div class="card-header">
          <div>
            <h2>Desktop updates</h2>
            <p>Packaged builds can discover, download, and install new desktop releases from GitHub.</p>
          </div>
          <span class="badge ${updateStatus.value.state === 'downloaded' ? 'badge-success' : 'badge-muted'}">
            ${escapeHtml(updateStatus.value.currentVersion)}
          </span>
        </div>
        <div class="callout">
          <strong>Status:</strong>
          <span>${escapeHtml(updateStatus.value.message ?? 'Ready to check for updates.')}</span>
        </div>
        ${
          updateStatus.value.percent !== null
            ? `
              <div class="progress-meta">
                <strong>${Math.round(updateStatus.value.percent)}%</strong>
                <span>${escapeHtml(`${formatBytes(updateStatus.value.transferredBytes)} / ${formatBytes(updateStatus.value.totalBytes)}`)}</span>
              </div>
            `
            : ''
        }
        <div class="button-row">
          <button class="btn btn-secondary" data-action="check-updates">Check for updates</button>
          ${
            updateStatus.value.state === 'available'
              ? '<button class="btn btn-primary" data-action="download-update">Download update</button>'
              : ''
          }
          ${
            updateStatus.value.state === 'downloaded'
              ? '<button class="btn btn-primary" data-action="install-update">Restart to install</button>'
              : ''
          }
        </div>
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

function renderUpdateActionButton(): string {
  const disabled = updateStatus.value.state === 'checking' || updateStatus.value.state === 'downloading';
  const action = disabled
    ? ''
    : updateStatus.value.state === 'downloaded'
      ? 'install-update'
      : updateStatus.value.state === 'available'
        ? 'download-update'
        : 'check-updates';
  const label =
    updateStatus.value.state === 'downloaded'
      ? 'Restart to update'
      : updateStatus.value.state === 'checking'
        ? 'Checking…'
      : updateStatus.value.state === 'available'
        ? 'Download update'
        : updateStatus.value.state === 'downloading'
          ? 'Downloading…'
          : 'Check updates';

  return `<button class="btn btn-secondary" ${action ? `data-action="${action}"` : 'disabled'}>${label}</button>`;
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
            <div class="status-card">
              <span class="status-title">Desktop update</span>
              <strong>${
                updateStatus.value.availableVersion
                  ? `v${escapeHtml(updateStatus.value.availableVersion)} ready`
                  : escapeHtml(updateStatus.value.currentVersion)
              }</strong>
              <span class="status-subtle">${escapeHtml(updateStatus.value.message ?? 'Ready to check for updates.')}</span>
            </div>
            <button class="btn btn-secondary" data-action="toggle-palette">Quick actions</button>
            ${renderUpdateActionButton()}
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

async function handleAction(action: string): Promise<void> {
  if (action === 'save') await saveSettings();
  if (action === 'start') await startBackend();
  if (action === 'stop') await stopBackend();
  if (action === 'open-browser') openLiveInBrowser();
  if (action === 'reload-live') reloadLiveView();
  if (action === 'refresh-logs') await refreshLogs();
  if (action === 'check-updates') await checkForDesktopUpdates();
  if (action === 'download-update') await downloadDesktopUpdate();
  if (action === 'install-update') await installDesktopUpdate();
  if (action === 'pick-executable') await chooseExecutable();
  if (action === 'pick-working-directory') await chooseWorkingDirectory();
  if (action === 'pick-opening-book') await chooseOpeningBook();
  if (action === 'pick-tablebase') await chooseTablebase();
  if (action === 'live') currentView.value = 'live';
  if (action === 'logs') currentView.value = 'logs';
  if (action === 'toggle-palette') paletteOpen.value = true;
  if (action === 'open-working-directory') {
    const path = desktopState.value.backendWorkingDirectory.trim();
    if (path) {
      await desktop.openPath(path);
    }
  }
}

function bindRootEvents(root: HTMLElement): void {
  root.addEventListener('click', (event) => {
    const target = (event.target as HTMLElement).closest<HTMLElement>(
      '[data-view], [data-action], [data-palette-action], [data-close-palette]',
    );
    if (!target) return;

    if (target.hasAttribute('data-close-palette')) {
      paletteOpen.value = false;
      return;
    }

    const view = target.dataset.view as DesktopView | undefined;
    if (view) {
      currentView.value = view;
      updateState({ lastView: view });
      void persistState(false);
      return;
    }

    const paletteAction = target.dataset.paletteAction;
    if (paletteAction) {
      paletteOpen.value = false;
      void handleAction(
        paletteAction === 'reload'
          ? 'reload-live'
          : paletteAction === 'browser'
            ? 'open-browser'
            : paletteAction,
      );
      return;
    }

    const action = target.dataset.action;
    if (action) {
      void handleAction(action);
    }
  });

  root.addEventListener('input', (event) => {
    const target = event.target;
    if (!(target instanceof HTMLInputElement)) return;

    if (target.id === 'backend-url') updateState({ backendUrl: target.value });
    if (target.id === 'backend-executable') updateState({ backendExecutable: target.value });
    if (target.id === 'backend-args') updateState({ backendArgs: target.value });
    if (target.id === 'backend-working-directory')
      updateState({ backendWorkingDirectory: target.value });
    if (target.id === 'opening-book-path') updateState({ openingBookPath: target.value });
    if (target.id === 'tablebase-path') updateState({ tablebasePath: target.value });
  });

  root.addEventListener('change', (event) => {
    const target = event.target;
    if (!(target instanceof HTMLInputElement)) return;

    if (target.id === 'auto-start') {
      updateState({ autoStartBackend: target.checked });
    }
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
      updateState({ lastView: currentView.value });
      void persistState(false);
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
  updateStatus.value = await desktop.getUpdateStatus();

  desktop.onBackendStatus((status) => {
    backendStatus.value = status;
  });
  desktop.onBackendLogs((logs) => {
    backendLogs.value = logs;
  });
  desktop.onUpdateStatus((status) => {
    updateStatus.value = status;
  });

  registerKeyboardShortcuts();
}

const appRoot = document.getElementById('app');
if (!appRoot) {
  throw new Error('Missing #app root element');
}

bindRootEvents(appRoot);

effect(() => {
  appRoot.innerHTML = renderApp();
});

void init();
