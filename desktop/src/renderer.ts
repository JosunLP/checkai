import './styles.css';

import { computed, effect, signal } from '@bquery/bquery/reactive';
import {
  DEFAULT_DESKTOP_STATE,
  type BackendStatusPayload,
  type DesktopApi,
  type DesktopState,
  type DesktopView,
  type UpdateStatusPayload,
} from './shared-types.js';

declare global {
  interface Window {
    checkaiDesktop?: DesktopApi;
  }
}

const fallbackApi: DesktopApi = {
  async getState() {
    return { ...DEFAULT_DESKTOP_STATE };
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
const DEFAULT_DESKTOP_VIEW: DesktopView = 'help';

const desktopState = signal<DesktopState>({ ...DEFAULT_DESKTOP_STATE });
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
let liveIframeElement: HTMLIFrameElement | null = null;
let liveIframeSrc = '';

interface InputSelectionState {
  id: string;
  selectionStart: number | null;
  selectionEnd: number | null;
  selectionDirection: 'forward' | 'backward' | 'none' | null;
}

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

function createLiveIframe(src: string): HTMLIFrameElement {
  const iframe = document.createElement('iframe');
  iframe.src = src;
  iframe.title = 'CheckAI engine workspace';
  iframe.setAttribute(
    'sandbox',
    'allow-downloads allow-forms allow-popups allow-same-origin allow-scripts',
  );
  return iframe;
}

function syncLiveIframe(host: ParentNode | null): void {
  if (!(host instanceof HTMLElement)) {
    return;
  }

  if (!canEmbedLiveView.value || !liveUrl.value) {
    host.replaceChildren();
    return;
  }

  if (!liveIframeElement) {
    liveIframeElement = createLiveIframe(liveUrl.value);
    liveIframeSrc = liveUrl.value;
  } else if (liveIframeSrc !== liveUrl.value) {
    liveIframeElement.src = liveUrl.value;
    liveIframeSrc = liveUrl.value;
  }

  host.replaceChildren(liveIframeElement);
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
      <div class="iframe-shell" data-live-iframe-host></div>
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
          ${renderUpdatePrimaryButton()}
        </div>
      </article>
    </section>
  `;
}

function renderMainContent(): string {
  const activeView = currentView.value;
  const renderers = {
    workspace: renderWorkspaceView,
    live: renderLiveView,
    engine: renderEngineView,
    logs: renderLogsView,
    help: renderHelpView,
  } satisfies Record<DesktopView, () => string>;
  const validViews = Object.keys(renderers) as DesktopView[];
  const renderedView: DesktopView =
    validViews.includes(activeView) ? activeView : DEFAULT_DESKTOP_VIEW;

  if (renderedView !== activeView) {
    console.warn(
      `Unknown desktop view "${activeView}", falling back to ${DEFAULT_DESKTOP_VIEW}. Valid views are: ${validViews.join(', ')}`,
    );
  }

  const content = renderers[renderedView]();

  return `<section class="view-panel active" data-view-panel="${renderedView}">${content}</section>`;
}

function getUpdatePrimaryAction(): { action: string; label: string; disabled: boolean } {
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

  return { action, label, disabled };
}

function renderUpdateButton(
  variant: 'primary' | 'secondary',
  options: { hideWhenDefaultCheck?: boolean } = {},
): string {
  const primary = getUpdatePrimaryAction();
  if (options.hideWhenDefaultCheck && primary.action === 'check-updates') {
    return '';
  }

  return `<button class="btn btn-${variant}" ${primary.action ? `data-action="${primary.action}"` : 'disabled'}>${primary.label}</button>`;
}

function renderUpdateActionButton(): string {
  return renderUpdateButton('secondary');
}

function renderUpdatePrimaryButton(): string {
  return renderUpdateButton('primary', { hideWhenDefaultCheck: true });
}

function captureFocusedInputState(root: HTMLElement): InputSelectionState | null {
  const activeElement = document.activeElement;
  if (!(activeElement instanceof HTMLInputElement) || !root.contains(activeElement) || !activeElement.id) {
    return null;
  }

  return {
    id: activeElement.id,
    selectionStart: activeElement.selectionStart,
    selectionEnd: activeElement.selectionEnd,
    selectionDirection: activeElement.selectionDirection,
  };
}

function restoreFocusedInputState(root: HTMLElement, state: InputSelectionState | null): void {
  if (!state) {
    return;
  }

  const nextInput = root.querySelector<HTMLInputElement>(`#${CSS.escape(state.id)}`);
  if (!nextInput) {
    return;
  }

  nextInput.focus({ preventScroll: true });
  if (state.selectionStart !== null && state.selectionEnd !== null) {
    nextInput.setSelectionRange(
      state.selectionStart,
      state.selectionEnd,
      state.selectionDirection ?? undefined,
    );
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
  try {
    switch (action) {
      case 'save':
        await saveSettings();
        break;
      case 'start':
        await startBackend();
        break;
      case 'stop':
        await stopBackend();
        break;
      case 'open-browser':
        openLiveInBrowser();
        break;
      case 'reload-live':
        reloadLiveView();
        break;
      case 'refresh-logs':
        await refreshLogs();
        break;
      case 'check-updates':
        await checkForDesktopUpdates();
        break;
      case 'download-update':
        await downloadDesktopUpdate();
        break;
      case 'install-update':
        await installDesktopUpdate();
        break;
      case 'pick-executable':
        await chooseExecutable();
        break;
      case 'pick-working-directory':
        await chooseWorkingDirectory();
        break;
      case 'pick-opening-book':
        await chooseOpeningBook();
        break;
      case 'pick-tablebase':
        await chooseTablebase();
        break;
      case 'live':
        currentView.value = 'live';
        break;
      case 'logs':
        currentView.value = 'logs';
        break;
      case 'toggle-palette':
        paletteOpen.value = true;
        break;
      case 'open-working-directory': {
        const path = desktopState.value.backendWorkingDirectory.trim();
        if (path) {
          await desktop.openPath(path);
        }
        break;
      }
    }
  } catch (error) {
    const text = error instanceof Error ? error.message : String(error);
    console.error(`Desktop action "${action}" failed:`, error);
    setMessage(text);
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
  const focusedInputState = captureFocusedInputState(appRoot);
  const template = document.createElement('template');
  template.innerHTML = renderApp();
  syncLiveIframe(template.content.querySelector('[data-live-iframe-host]'));
  appRoot.replaceChildren(template.content);
  restoreFocusedInputState(appRoot, focusedInputState);
});

void init();
