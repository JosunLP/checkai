import './styles.scss';

import { component, html, safeHtml } from '@bquery/bquery/component';
import { computed, effect, signal } from '@bquery/bquery/reactive';
import {
  cancelAnalysisJob as apiCancelAnalysisJob,
  createGame as apiCreateGame,
  deleteGame as apiDeleteGame,
  exportFen as apiExportFen,
  exportPgn as apiExportPgn,
  getAnalysisJob as apiGetAnalysisJob,
  getBoardAscii as apiGetBoardAscii,
  getGame as apiGetGame,
  getLegalMoves as apiGetLegalMoves,
  getStorageStats as apiGetStorageStats,
  importFen as apiImportFen,
  listAnalysisJobs as apiListAnalysisJobs,
  listArchived as apiListArchived,
  listGames as apiListGames,
  replayArchived as apiReplayArchived,
  startAnalysis as apiStartAnalysis,
  submitAction as apiSubmitAction,
  submitMove as apiSubmitMove,
  setApiBase,
} from './api-client.js';
import {
  DEFAULT_DESKTOP_STATE,
  FILES,
  PIECE_UNICODE,
  RANKS,
  type AnalysisJob,
  type AnalysisResultPayload,
  type ArchivedGameSummary,
  type BackendPreset,
  type BackendStatusPayload,
  type BoardMap,
  type DesktopApi,
  type DesktopState,
  type DesktopView,
  type FenChar,
  type Game,
  type GameSummary,
  type LegalMove,
  type ReplayState,
  type SaveTextFileOptions,
  type StorageStats,
  type UpdateStatusPayload,
} from './shared-types.js';

declare global {
  interface Window {
    checkaiDesktop?: DesktopApi;
  }
}

// ── Fallback API for running outside Electron ───────────────────────────────

const fallbackApi: DesktopApi = {
  async getState() {
    return { ...DEFAULT_DESKTOP_STATE };
  },
  async saveState(s) {
    return s;
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
    return 'Open this build inside Electron to use native features.';
  },
  async startBackend() {
    return fallbackApi.getBackendStatus();
  },
  async stopBackend() {
    return fallbackApi.getBackendStatus();
  },
  async getUpdateStatus() {
    return {
      supported: false,
      currentVersion: 'dev',
      state: 'unsupported' as const,
      availableVersion: null,
      percent: null,
      transferredBytes: null,
      totalBytes: null,
      message: 'Desktop updates are available in packaged builds.',
    };
  },
  async setProgressBar() {},
  async checkForUpdates() {
    return fallbackApi.getUpdateStatus();
  },
  async downloadUpdate() {
    return fallbackApi.getUpdateStatus();
  },
  async installUpdate() {},
  async pickFile() {
    return null;
  },
  async pickDirectory() {
    return null;
  },
  async readTextFile() {
    throw new Error('Local file reading is only available inside Electron.');
  },
  async saveTextFile() {
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
  onMenuCommand() {
    return () => undefined;
  },
};

const desktop = window.checkaiDesktop ?? fallbackApi;

// ── Reactive state ──────────────────────────────────────────────────────────

const desktopState = signal<DesktopState>({ ...DEFAULT_DESKTOP_STATE });
const currentView = signal<DesktopView>('dashboard');
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
  message: null,
});
const paletteOpen = signal(false);
const paletteQuery = signal('');
const toastMsg = signal<string | null>(null);
const errorMsg = signal<string | null>(null);
const boardAscii = signal('');
const liveConnection = signal<'connecting' | 'connected' | 'disconnected'>(
  'disconnected'
);
const liveMessage = signal('Live sync offline');
const importDropActive = signal(false);

// Engine data
const gamesList = signal<GameSummary[]>([]);
const activeGame = signal<Game | null>(null);
const legalMoves = signal<LegalMove[]>([]);
const selectedSquare = signal<string | null>(null);
const archivedList = signal<ArchivedGameSummary[]>([]);
const storageStats = signal<StorageStats | null>(null);
const replayState = signal<ReplayState | null>(null);
const replayGameId = signal<string | null>(null);
const analysisJobs = signal<AnalysisJob[]>([]);
const activeAnalysis = signal<AnalysisJob | null>(null);
const fenInput = signal('');
const analysisDepth = signal(30);
const analysisPolling = signal(false);

const notifiedAnalysisJobs = new Set<string>();
let liveWs: WebSocket | null = null;
let liveReconnectTimer: ReturnType<typeof setTimeout> | null = null;
let liveReconnectDelay = 1000;
let liveSubscribedGameId: string | null = null;
const LIVE_RECONNECT_MAX_DELAY = 30000;

// Computed
const boardFlipped = computed(() => desktopState.value.boardFlipped);

const highlightSquares = computed((): Set<string> => {
  const sq = selectedSquare.value;
  if (!sq) return new Set();
  return new Set(
    legalMoves.value.filter((m) => m.from === sq).map((m) => m.to)
  );
});

const lastMove = computed((): { from: string; to: string } | null => {
  const g = activeGame.value;
  if (!g || g.move_history.length === 0) return null;
  const last = g.move_history[g.move_history.length - 1];
  return { from: last.move_json.from, to: last.move_json.to };
});

const currentWorkspace = computed(
  () =>
    desktopState.value.backendWorkingDirectory.trim() || 'No workspace selected'
);

const backendUptimeLabel = computed(() => {
  const startedAt = backendStatus.value.startedAt;
  if (!backendStatus.value.running || !startedAt) return '—';
  const totalSeconds = Math.max(0, Math.floor((Date.now() - startedAt) / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes}m ${seconds}s`;
  return `${seconds}s`;
});

const filteredPaletteActions = computed(() => {
  const query = paletteQuery.value.trim().toLowerCase();
  const actions = [
    { id: 'create-game', label: 'New game', meta: 'Start a fresh game' },
    {
      id: 'start-backend',
      label: 'Start backend',
      meta: 'Launch local engine',
    },
    { id: 'stop-backend', label: 'Stop backend', meta: 'Stop local engine' },
    {
      id: 'refresh-logs',
      label: 'Refresh logs',
      meta: 'Reload backend console output',
    },
    {
      id: 'import-fen-file',
      label: 'Import FEN file',
      meta: 'Load a FEN from disk',
    },
    {
      id: 'save-settings',
      label: 'Save workspace',
      meta: 'Persist desktop preferences',
    },
    {
      id: 'nav:dashboard',
      label: 'Dashboard',
      meta: 'Overview and quick actions',
    },
    { id: 'nav:games', label: 'Games', meta: 'Active games and FEN import' },
    { id: 'nav:archive', label: 'Archive', meta: 'Replay completed games' },
    { id: 'nav:analysis', label: 'Analysis', meta: 'Engine analysis jobs' },
    {
      id: 'nav:engine',
      label: 'Engine config',
      meta: 'Backend presets and assets',
    },
    { id: 'nav:logs', label: 'Logs', meta: 'Backend and debug output' },
    { id: 'nav:settings', label: 'Settings', meta: 'Desktop preferences' },
    {
      id: 'check-updates',
      label: 'Check updates',
      meta: 'Look for a newer desktop build',
    },
    { id: 'flip-board', label: 'Flip board', meta: 'Swap board orientation' },
  ];

  if (!query) return actions;
  return actions.filter(
    (action) =>
      action.label.toLowerCase().includes(query) ||
      action.meta.toLowerCase().includes(query)
  );
});

// ── Helpers ─────────────────────────────────────────────────────────────────

function escapeHtml(v: string): string {
  return v
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');
}

function formatBytes(v: number | null): string {
  if (v === null || !Number.isFinite(v)) return '—';
  if (v < 1024) return `${v} B`;
  const units = ['KB', 'MB', 'GB'];
  let s = v / 1024;
  let i = 0;
  while (s >= 1024 && i < units.length - 1) {
    s /= 1024;
    i++;
  }
  return `${s.toFixed(s >= 10 ? 0 : 1)} ${units[i]}`;
}

function formatDateTime(value: number | null | undefined): string {
  if (!value) return '—';
  return new Date(value).toLocaleString();
}

function basename(input: string): string {
  const normalized = input.replace(/[\\/]+$/, '');
  if (!normalized) return input;
  const parts = normalized.split(/[\\/]/);
  return parts[parts.length - 1] || normalized;
}

function toSlug(input: string): string {
  return (
    input
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '') || 'checkai-export'
  );
}

function buildExportOptions(
  defaultPath: string,
  content: string,
  filters: SaveTextFileOptions['filters']
): SaveTextFileOptions {
  return { defaultPath, content, filters };
}

function normalizeFenText(raw: string): string {
  return (
    raw
      .replace(/^\uFEFF/, '')
      .trim()
      .split(/\r?\n/)[0]
      ?.trim() ?? ''
  );
}

function syncTheme(theme: DesktopState['theme']): void {
  if (theme === 'system') {
    const prefersLight = window.matchMedia(
      '(prefers-color-scheme: light)'
    ).matches;
    document.documentElement.setAttribute(
      'data-theme',
      prefersLight ? 'light' : 'dark'
    );
    return;
  }
  document.documentElement.setAttribute('data-theme', theme);
}

function showToast(msg: string): void {
  toastMsg.value = msg;
  setTimeout(() => {
    if (toastMsg.value === msg) toastMsg.value = null;
  }, 3000);
}

function showError(msg: string): void {
  errorMsg.value = msg;
  setTimeout(() => {
    if (errorMsg.value === msg) errorMsg.value = null;
  }, 6000);
}

function qualityColor(q: string): string {
  switch (q) {
    case 'Best':
      return '#34d399';
    case 'Excellent':
      return '#6ee7b7';
    case 'Good':
      return '#a3e635';
    case 'Inaccuracy':
      return '#fbbf24';
    case 'Mistake':
      return '#fb923c';
    case 'Blunder':
      return '#fb7185';
    case 'Book':
      return '#94a3b8';
    default:
      return '#94a3b8';
  }
}

function resultLabel(r: string | null): string {
  if (r === 'WhiteWins') return '1-0';
  if (r === 'BlackWins') return '0-1';
  if (r === 'Draw') return '½-½';
  return '*';
}

async function notifyUser(title: string, body: string): Promise<void> {
  if (!desktopState.value.notificationsEnabled) return;
  try {
    await desktop.notify(title, body);
  } catch {
    // Best-effort only.
  }
}

function updateRecentWorkspaces(path: string): void {
  const normalized = path.trim();
  if (!normalized) return;
  desktopState.value = {
    ...desktopState.value,
    recentWorkspaces: [
      normalized,
      ...desktopState.value.recentWorkspaces.filter(
        (entry) => entry !== normalized
      ),
    ].slice(0, 8),
  };
}

function buildPresetFromState(name: string): BackendPreset {
  return {
    id: crypto.randomUUID(),
    name,
    backendExecutable: desktopState.value.backendExecutable,
    backendArgs: desktopState.value.backendArgs,
    backendWorkingDirectory: desktopState.value.backendWorkingDirectory,
    backendUrl: desktopState.value.backendUrl,
    openingBookPath: desktopState.value.openingBookPath,
    tablebasePath: desktopState.value.tablebasePath,
    autoStartBackend: desktopState.value.autoStartBackend,
    createdAt: Date.now(),
  };
}

function loadPresetIntoState(presetId: string): void {
  const preset = desktopState.value.backendPresets.find(
    (entry) => entry.id === presetId
  );
  if (!preset) return;
  desktopState.value = {
    ...desktopState.value,
    backendExecutable: preset.backendExecutable,
    backendArgs: preset.backendArgs,
    backendWorkingDirectory: preset.backendWorkingDirectory,
    backendUrl: preset.backendUrl,
    openingBookPath: preset.openingBookPath,
    tablebasePath: preset.tablebasePath,
    autoStartBackend: preset.autoStartBackend,
  };
  if (preset.backendWorkingDirectory) {
    updateRecentWorkspaces(preset.backendWorkingDirectory);
  }
  showToast(`Preset “${preset.name}” loaded`);
}

function removePresetFromState(presetId: string): void {
  const preset = desktopState.value.backendPresets.find(
    (entry) => entry.id === presetId
  );
  desktopState.value = {
    ...desktopState.value,
    backendPresets: desktopState.value.backendPresets.filter(
      (entry) => entry.id !== presetId
    ),
  };
  if (preset) showToast(`Preset “${preset.name}” removed`);
}

function saveCurrentPreset(): void {
  const workspaceName = desktopState.value.backendWorkingDirectory
    ? basename(desktopState.value.backendWorkingDirectory)
    : 'Local engine';
  const preset = buildPresetFromState(workspaceName);
  desktopState.value = {
    ...desktopState.value,
    backendPresets: [preset, ...desktopState.value.backendPresets].slice(0, 12),
  };
  showToast(`Preset “${preset.name}” saved`);
}

function liveWsUrl(): string | null {
  try {
    const url = new URL(desktopState.value.backendUrl);
    url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
    url.pathname = '/ws';
    url.search = '';
    url.hash = '';
    return url.toString();
  } catch {
    return null;
  }
}

function liveSend(payload: Record<string, unknown>): void {
  if (liveWs?.readyState === WebSocket.OPEN) {
    liveWs.send(JSON.stringify(payload));
  }
}

function subscribeLiveGame(gameId: string | null): void {
  if (liveSubscribedGameId && liveSubscribedGameId !== gameId) {
    liveSend({ action: 'unsubscribe', game_id: liveSubscribedGameId });
  }
  liveSubscribedGameId = gameId;
  if (gameId) {
    liveSend({ action: 'subscribe', game_id: gameId });
  }
}

function scheduleLiveReconnect(): void {
  if (liveReconnectTimer) return;
  liveReconnectTimer = setTimeout(() => {
    liveReconnectTimer = null;
    connectLiveUpdates();
  }, liveReconnectDelay);
  liveReconnectDelay = Math.min(
    liveReconnectDelay * 2,
    LIVE_RECONNECT_MAX_DELAY
  );
}

function handleLiveMessage(raw: unknown): void {
  if (typeof raw !== 'string') return;

  try {
    const message = JSON.parse(raw) as {
      type?: string;
      event?: string;
      game_id?: string;
    };

    if (message.type !== 'event') return;

    if (message.event === 'game_updated' && message.game_id) {
      if (activeGame.value?.game_id === message.game_id) {
        void openGame(message.game_id, true);
      }
      void refreshGamesList();
      void refreshArchive();
      return;
    }

    if (message.event === 'game_created' || message.event === 'game_deleted') {
      void refreshGamesList();
      void refreshArchive();
    }
  } catch {
    // Ignore malformed messages.
  }
}

function connectLiveUpdates(): void {
  const url = liveWsUrl();
  if (!url) {
    liveConnection.value = 'disconnected';
    liveMessage.value = 'Live sync unavailable: invalid backend URL';
    return;
  }

  if (
    liveWs &&
    (liveWs.readyState === WebSocket.OPEN ||
      liveWs.readyState === WebSocket.CONNECTING)
  ) {
    return;
  }

  liveConnection.value = 'connecting';
  liveMessage.value = 'Connecting live sync…';

  try {
    liveWs = new WebSocket(url);
  } catch {
    liveConnection.value = 'disconnected';
    liveMessage.value = 'Live sync failed to start';
    scheduleLiveReconnect();
    return;
  }

  liveWs.onopen = () => {
    liveConnection.value = 'connected';
    liveMessage.value = 'Live sync active';
    liveReconnectDelay = 1000;
    if (liveReconnectTimer) {
      clearTimeout(liveReconnectTimer);
      liveReconnectTimer = null;
    }
    subscribeLiveGame(
      activeGame.value?.game_id ?? desktopState.value.lastGameId ?? null
    );
  };

  liveWs.onmessage = (event) => {
    handleLiveMessage(event.data);
  };

  liveWs.onclose = () => {
    liveConnection.value = 'disconnected';
    liveMessage.value = 'Live sync disconnected';
    liveWs = null;
    scheduleLiveReconnect();
  };

  liveWs.onerror = () => {
    liveConnection.value = 'disconnected';
    liveMessage.value = 'Live sync error';
  };
}

// ── API Interactions ────────────────────────────────────────────────────────

async function refreshGamesList(): Promise<void> {
  try {
    const res = await apiListGames();
    gamesList.value = res.games;
  } catch (e) {
    showError((e as Error).message);
  }
}

async function createNewGame(): Promise<void> {
  try {
    const res = await apiCreateGame();
    await refreshGamesList();
    await openGame(res.game_id);
    showToast('New game created');
  } catch (e) {
    showError((e as Error).message);
  }
}

async function importFenText(fenText: string): Promise<void> {
  const fen = normalizeFenText(fenText);
  if (!fen) {
    showError('No FEN found to import.');
    return;
  }

  try {
    const res = await apiImportFen(fen);
    fenInput.value = '';
    await refreshGamesList();
    await openGame(res.game_id);
    showToast('Game imported from FEN');
  } catch (e) {
    showError((e as Error).message);
  }
}

async function importFromFen(): Promise<void> {
  const fen = fenInput.value.trim();
  if (!fen) return;
  await importFenText(fen);
}

async function refreshBoardAscii(gameId: string): Promise<void> {
  try {
    boardAscii.value = await apiGetBoardAscii(gameId);
  } catch {
    boardAscii.value = 'Board ASCII preview unavailable.';
  }
}

async function openGame(id: string, keepCurrentView = false): Promise<void> {
  try {
    const g = await apiGetGame(id);
    activeGame.value = g;
    selectedSquare.value = null;
    boardAscii.value = '';
    updateDesktopState({ lastGameId: id });
    subscribeLiveGame(id);
    if (!g.is_over) {
      const res = await apiGetLegalMoves(id);
      legalMoves.value = res.moves;
    } else {
      legalMoves.value = [];
    }
    await refreshBoardAscii(id);
    if (!keepCurrentView) currentView.value = 'board';
  } catch (e) {
    showError((e as Error).message);
  }
}

async function handleSquareClick(sq: string): Promise<void> {
  const g = activeGame.value;
  if (!g || g.is_over) return;

  const sel = selectedSquare.value;
  if (sel) {
    const move = legalMoves.value.find((m) => m.from === sel && m.to === sq);
    if (move) {
      try {
        const res = await apiSubmitMove(g.game_id, sel, sq, move.promotion);
        if (res.success) {
          await openGame(g.game_id);
        } else {
          showError(res.message);
        }
      } catch (e) {
        showError((e as Error).message);
      }
      selectedSquare.value = null;
      return;
    }
  }

  const piece = g.state.board[sq];
  if (piece) {
    const isWhitePiece = piece === piece.toUpperCase();
    const isWhiteTurn = g.state.turn === 'white';
    if (isWhitePiece === isWhiteTurn) {
      selectedSquare.value = sq;
      return;
    }
  }
  selectedSquare.value = null;
}

async function doDeleteGame(id: string): Promise<void> {
  try {
    await apiDeleteGame(id);
    if (activeGame.value?.game_id === id) {
      activeGame.value = null;
      legalMoves.value = [];
      boardAscii.value = '';
      subscribeLiveGame(null);
    }
    await refreshGamesList();
    showToast('Game deleted');
  } catch (e) {
    showError((e as Error).message);
  }
}

async function doAction(action: string, reason?: string): Promise<void> {
  const g = activeGame.value;
  if (!g) return;
  try {
    await apiSubmitAction(g.game_id, action, reason);
    await openGame(g.game_id);
  } catch (e) {
    showError((e as Error).message);
  }
}

async function doExportFen(): Promise<void> {
  const g = activeGame.value;
  if (!g) return;
  try {
    const res = await apiExportFen(g.game_id);
    await navigator.clipboard.writeText(res.fen);
    showToast('FEN copied to clipboard');
  } catch (e) {
    showError((e as Error).message);
  }
}

async function saveTextExport(
  defaultPath: string,
  content: string,
  filters: SaveTextFileOptions['filters'],
  successMessage: string
): Promise<void> {
  try {
    const savedPath = await desktop.saveTextFile(
      buildExportOptions(defaultPath, content, filters)
    );
    if (savedPath) {
      showToast(successMessage);
      await notifyUser(
        'CheckAI Desktop',
        `${successMessage} (${basename(savedPath)})`
      );
    }
  } catch (e) {
    showError((e as Error).message);
  }
}

async function doSaveFen(): Promise<void> {
  const g = activeGame.value;
  if (!g) return;
  try {
    const res = await apiExportFen(g.game_id);
    await saveTextExport(
      `${toSlug(g.game_id)}.fen`,
      res.fen,
      [{ name: 'FEN position', extensions: ['fen', 'txt'] }],
      'FEN saved to disk'
    );
  } catch (e) {
    showError((e as Error).message);
  }
}

async function doExportPgn(): Promise<void> {
  const g = activeGame.value;
  if (!g) return;
  try {
    const pgn = await apiExportPgn(g.game_id);
    await navigator.clipboard.writeText(pgn);
    showToast('PGN copied to clipboard');
  } catch (e) {
    showError((e as Error).message);
  }
}

async function doSavePgn(): Promise<void> {
  const g = activeGame.value;
  if (!g) return;
  try {
    const pgn = await apiExportPgn(g.game_id);
    await saveTextExport(
      `${toSlug(g.game_id)}.pgn`,
      pgn,
      [{ name: 'PGN game', extensions: ['pgn', 'txt'] }],
      'PGN saved to disk'
    );
  } catch (e) {
    showError((e as Error).message);
  }
}

async function importFenFromFile(): Promise<void> {
  try {
    const filePath = await desktop.pickFile();
    if (!filePath) return;
    const text = await desktop.readTextFile(filePath);
    await importFenText(text);
  } catch (e) {
    showError((e as Error).message);
  }
}

// ── Archive ─────────────────────────────────────────────────────────────────

async function refreshArchive(): Promise<void> {
  try {
    const res = await apiListArchived();
    archivedList.value = res.games;
    if (res.storage) storageStats.value = res.storage;
  } catch (e) {
    showError((e as Error).message);
  }
}

async function openArchivedGame(id: string): Promise<void> {
  try {
    replayGameId.value = id;
    const rs = await apiReplayArchived(id, 0);
    replayState.value = rs;
    currentView.value = 'archive';
  } catch (e) {
    showError((e as Error).message);
  }
}

async function replayTo(moveNum: number): Promise<void> {
  const id = replayGameId.value;
  if (!id) return;
  try {
    replayState.value = await apiReplayArchived(id, moveNum);
  } catch (e) {
    showError((e as Error).message);
  }
}

// ── Analysis ────────────────────────────────────────────────────────────────

let analysisPollTimer: ReturnType<typeof setInterval> | null = null;

async function refreshAnalysisJobs(): Promise<void> {
  try {
    const res = await apiListAnalysisJobs();
    analysisJobs.value = res.jobs;
  } catch (e) {
    showError((e as Error).message);
  }
}

async function submitAnalysis(gameId: string): Promise<void> {
  try {
    const res = await apiStartAnalysis(gameId, analysisDepth.value);
    showToast('Analysis started');
    await refreshAnalysisJobs();
    await pollAnalysisJob(res.job_id);
  } catch (e) {
    showError((e as Error).message);
  }
}

async function pollAnalysisJob(jobId: string): Promise<void> {
  if (analysisPollTimer) {
    clearInterval(analysisPollTimer);
    analysisPollTimer = null;
  }
  analysisPolling.value = true;
  const poll = async () => {
    try {
      const job = await apiGetAnalysisJob(jobId);
      activeAnalysis.value = job;
      if (
        job.status === 'Completed' ||
        job.status === 'Cancelled' ||
        (typeof job.status === 'object' && 'Failed' in job.status)
      ) {
        if (analysisPollTimer) {
          clearInterval(analysisPollTimer);
          analysisPollTimer = null;
        }
        analysisPolling.value = false;
        if (!notifiedAnalysisJobs.has(job.id)) {
          notifiedAnalysisJobs.add(job.id);
          if (job.status === 'Completed') {
            await notifyUser(
              'CheckAI analysis complete',
              `Job ${job.id.slice(0, 8)} finished successfully.`
            );
          }
          if (typeof job.status === 'object' && 'Failed' in job.status) {
            await notifyUser(
              'CheckAI analysis failed',
              job.status.Failed.error
            );
          }
        }
        await refreshAnalysisJobs();
      }
    } catch {
      if (analysisPollTimer) {
        clearInterval(analysisPollTimer);
        analysisPollTimer = null;
      }
      analysisPolling.value = false;
    }
  };
  await poll();
  if (analysisPolling.value) {
    analysisPollTimer = setInterval(poll, 1500);
  }
}

async function cancelAnalysis(jobId: string): Promise<void> {
  try {
    await apiCancelAnalysisJob(jobId);
    await refreshAnalysisJobs();
    if (activeAnalysis.value?.id === jobId) activeAnalysis.value = null;
    showToast('Analysis cancelled');
  } catch (e) {
    showError((e as Error).message);
  }
}

// ── Desktop actions ─────────────────────────────────────────────────────────

function updateDesktopState(patch: Partial<DesktopState>): void {
  desktopState.value = { ...desktopState.value, ...patch };
}

async function saveDesktopSettings(): Promise<void> {
  if (desktopState.value.backendWorkingDirectory.trim()) {
    updateRecentWorkspaces(desktopState.value.backendWorkingDirectory);
  }
  const state = { ...desktopState.value, lastView: currentView.value };
  desktopState.value = await desktop.saveState(state);
  showToast('Settings saved');
}

async function persistViewSilently(): Promise<void> {
  const state = { ...desktopState.value, lastView: currentView.value };
  desktopState.value = await desktop.saveState(state);
}

async function startBackend(): Promise<void> {
  if (desktopState.value.backendWorkingDirectory.trim()) {
    updateRecentWorkspaces(desktopState.value.backendWorkingDirectory);
  }
  const state = { ...desktopState.value, lastView: currentView.value };
  backendStatus.value = await desktop.startBackend(state);
  setApiBase(desktopState.value.backendUrl);
  connectLiveUpdates();
}

async function stopBackend(): Promise<void> {
  backendStatus.value = await desktop.stopBackend();
}

async function refreshLogs(): Promise<void> {
  backendLogs.value = await desktop.getBackendLogs();
}

// ── Board rendering ─────────────────────────────────────────────────────────

function renderBoard(
  board: BoardMap,
  flipped: boolean,
  interactive: boolean
): string {
  const ranks = flipped ? [...RANKS] : [...RANKS].reverse();
  const files = flipped ? [...FILES].reverse() : [...FILES];
  const sel = selectedSquare.value;
  const highlights = highlightSquares.value;
  const lm = lastMove.value;
  const check = activeGame.value?.is_check ?? false;
  const turn = activeGame.value?.state.turn;
  let kingSquare: string | null = null;
  if (check && turn) {
    const kingChar = turn === 'white' ? 'K' : 'k';
    for (const sq of Object.keys(board)) {
      if (board[sq] === kingChar) {
        kingSquare = sq;
        break;
      }
    }
  }

  let rows = '';
  for (const r of ranks) {
    let cells = '';
    for (const f of files) {
      const sq = `${f}${r}`;
      const piece = board[sq] as FenChar | null;
      const isLight = (f.charCodeAt(0) + parseInt(r)) % 2 === 0;
      const classes = [
        'sq',
        isLight ? 'sq-light' : 'sq-dark',
        sel === sq ? 'sq-selected' : '',
        highlights.has(sq) ? 'sq-highlight' : '',
        lm && (lm.from === sq || lm.to === sq) ? 'sq-last-move' : '',
        kingSquare === sq ? 'sq-check' : '',
      ]
        .filter(Boolean)
        .join(' ');
      const pieceStr = piece ? PIECE_UNICODE[piece] : '';
      const cursor = interactive ? 'cursor:pointer;' : '';
      cells += `<div class="${classes}" data-sq="${sq}" style="${cursor}">${pieceStr}</div>`;
    }
    rows += `<div class="board-row"><span class="rank-label">${r}</span>${cells}</div>`;
  }
  let fileLabels = '<div class="file-labels"><span></span>';
  for (const f of files) fileLabels += `<span>${f}</span>`;
  fileLabels += '</div>';
  return `<div class="chess-board">${rows}${fileLabels}</div>`;
}

// ── Component: Dashboard ────────────────────────────────────────────────────

component('cai-dashboard', {
  styles: `:host { display:block; }`,
  render() {
    const bs = backendStatus.value;
    const us = updateStatus.value;
    const games = gamesList.value;
    const stats = storageStats.value;
    const presets = desktopState.value.backendPresets;
    const recentWorkspaces = desktopState.value.recentWorkspaces;

    return html`
      <div class="view-grid">
        <div class="card hero-card">
          <div class="card-head">
            <div>
              <h2>♔ CheckAI Desktop</h2>
              <p class="dim">
                Workspace-first control room for the CheckAI chess engine.
              </p>
            </div>
            <span class="badge ${bs.running ? 'badge-ok' : 'badge-dim'}"
              >${bs.running ? '● Engine online' : '○ Engine offline'}</span
            >
          </div>
          <div class="hero-meta">
            <div class="hero-stat">
              <span class="stat-label">Workspace</span>
              <strong>${safeHtml`${currentWorkspace.value}`}</strong>
            </div>
            <div class="hero-stat">
              <span class="stat-label">Live sync</span>
              <strong>${safeHtml`${liveMessage.value}`}</strong>
            </div>
            <div class="hero-stat">
              <span class="stat-label">Saved presets</span>
              <strong>${presets.length}</strong>
            </div>
          </div>
          <div class="quick-strip">
            <button class="qbtn" data-action="create-game">
              <strong>♟ New game</strong
              ><span>Start a fresh game from the starting position</span>
            </button>
            <button class="qbtn" data-action="nav:games">
              <strong>♜ Active games</strong
              ><span>Browse, open, or manage running games</span>
            </button>
            <button class="qbtn" data-action="nav:archive">
              <strong>📦 Archive</strong
              ><span>Review completed games and replay any position</span>
            </button>
            <button class="qbtn" data-action="nav:analysis">
              <strong>📊 Analysis</strong
              ><span>Deep engine analysis on any completed game</span>
            </button>
            <button class="qbtn" data-action="import-fen-file">
              <strong>📂 Import from file</strong
              ><span>Load a FEN from disk with native file dialogs</span>
            </button>
            <button class="qbtn" data-action="nav:engine">
              <strong>⚙ Workspace presets</strong
              ><span>Switch backend profiles, assets, and working folders</span>
            </button>
          </div>
        </div>
        <div class="card">
          <div class="card-head">
            <div>
              <h3>⚡ Backend status</h3>
              <p class="dim">
                Local engine process health and runtime details.
              </p>
            </div>
            <span class="badge ${bs.running ? 'badge-ok' : 'badge-danger'}"
              >${bs.running ? 'Running' : 'Stopped'}</span
            >
          </div>
          <div class="stat-grid">
            <div class="stat">
              <span class="stat-label">PID</span
              ><strong>${bs.pid ?? '—'}</strong>
            </div>
            <div class="stat">
              <span class="stat-label">Command</span
              ><strong class="mono">${safeHtml`${bs.command ?? '—'}`}</strong>
            </div>
            <div class="stat">
              <span class="stat-label">Version</span
              ><strong>${safeHtml`${us.currentVersion}`}</strong>
            </div>
            <div class="stat">
              <span class="stat-label">Uptime</span
              ><strong>${backendUptimeLabel.value}</strong>
            </div>
          </div>
          <div class="btn-row" style="margin-top:1rem">
            <button class="btn btn-primary btn-sm" data-action="start-backend">
              ▶ Start
            </button>
            <button class="btn btn-ghost btn-sm" data-action="stop-backend">
              ■ Stop
            </button>
          </div>
        </div>
        <div class="card">
          <div class="card-head">
            <div>
              <h3>📂 Recent workspaces</h3>
              <p class="dim">Jump between productive local setups.</p>
            </div>
          </div>
          ${recentWorkspaces.length === 0
            ? html`<p class="empty-text">
                Choose a working directory to build your first desktop
                workspace.
              </p>`
            : html`<div class="mini-list">
                ${recentWorkspaces
                  .map(
                    (workspace) => html`
                      <button
                        class="mini-item"
                        data-action="use-recent-workspace"
                        data-id="${workspace}"
                      >
                        <span>${safeHtml`${basename(workspace)}`}</span>
                        <span class="mono dim">${safeHtml`${workspace}`}</span>
                      </button>
                    `
                  )
                  .join('')}
              </div>`}
        </div>
        <div class="card">
          <div class="card-head">
            <div>
              <h3>♟ Active games</h3>
              <p class="dim">
                ${games.length} game${games.length !== 1 ? 's' : ''} in progress
              </p>
            </div>
            <button class="btn btn-ghost btn-sm" data-action="nav:games">
              View all →
            </button>
          </div>
          ${games.length === 0
            ? html`<p class="empty-text">
                No active games. Create one to get started.
              </p>`
            : html`<div class="mini-list">
                ${games
                  .slice(0, 5)
                  .map(
                    (g) => html`
                      <button
                        class="mini-item"
                        data-action="open-game"
                        data-id="${g.game_id}"
                      >
                        <span class="mono">${g.game_id.slice(0, 8)}</span>
                        <span
                          >${g.turn} · Move
                          ${g.fullmove_number}${g.is_over
                            ? ' · ' + resultLabel(g.result)
                            : ''}</span
                        >
                      </button>
                    `
                  )
                  .join('')}
              </div>`}
        </div>
        <div class="card">
          <div class="card-head">
            <div>
              <h3>💾 Storage</h3>
              <p class="dim">Disk usage for active and archived games.</p>
            </div>
          </div>
          ${stats
            ? html`<div class="stat-grid">
                <div class="stat">
                  <span class="stat-label">Active</span
                  ><strong
                    >${stats.active_count} ·
                    ${formatBytes(stats.active_bytes)}</strong
                  >
                </div>
                <div class="stat">
                  <span class="stat-label">Archived</span
                  ><strong
                    >${stats.archived_count} ·
                    ${formatBytes(stats.archive_bytes)}</strong
                  >
                </div>
              </div>`
            : html`<p class="empty-text">
                Start the backend to view storage statistics.
              </p>`}
        </div>
        <div class="card">
          <div class="card-head">
            <div>
              <h3>🔖 Saved presets</h3>
              <p class="dim">
                Reusable backend launch profiles for common workflows.
              </p>
            </div>
            <button class="btn btn-ghost btn-sm" data-action="nav:engine">
              Manage →
            </button>
          </div>
          ${presets.length === 0
            ? html`<p class="empty-text">
                Save your current backend settings as a preset from the Engine
                view.
              </p>`
            : html`<div class="mini-list">
                ${presets
                  .slice(0, 4)
                  .map(
                    (preset) => html`
                      <button
                        class="mini-item"
                        data-action="load-preset"
                        data-id="${preset.id}"
                      >
                        <span>${safeHtml`${preset.name}`}</span>
                        <span class="dim"
                          >${safeHtml`${preset.backendArgs || 'serve'}`}</span
                        >
                      </button>
                    `
                  )
                  .join('')}
              </div>`}
        </div>
      </div>
    `;
  },
});

// ── Component: Games List ───────────────────────────────────────────────────

component('cai-games', {
  styles: `:host { display:block; }`,
  render() {
    const games = gamesList.value;
    return html`
      <div class="card">
        <div class="card-head">
          <div>
            <h2>♟ Active games</h2>
            <p class="dim">
              ${games.length} game${games.length !== 1 ? 's' : ''} loaded ·
              Create, open, or delete running games.
            </p>
          </div>
          <div class="btn-row">
            <button class="btn btn-primary btn-sm" data-action="create-game">
              ✚ New game
            </button>
            <button class="btn btn-sm" data-action="import-fen-file">
              📂 Import file
            </button>
            <button class="btn btn-ghost btn-sm" data-action="refresh-games">
              ↻ Refresh
            </button>
          </div>
        </div>
        <div class="fen-row">
          <label class="field-inline">
            <span>Import FEN:</span>
            <input
              id="fen-import"
              placeholder="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
              value="${escapeHtml(fenInput.value)}"
            />
          </label>
          <button class="btn btn-sm" data-action="import-fen">Import</button>
        </div>
        <div
          class="drop-zone ${importDropActive.value ? 'drop-zone-active' : ''}"
          data-drop-zone
        >
          <span style="font-size:1.5rem">📄</span>
          <strong>Drag & drop a FEN file</strong>
          <span class="dim"
            >Drop plain text, .fen, or .txt files anywhere in this panel.</span
          >
        </div>
        ${games.length === 0
          ? html`<p class="empty-text">No active games yet.</p>`
          : html`<div class="table-wrap">
              <table class="data-table">
                <thead>
                  <tr>
                    <th>ID</th>
                    <th>Turn</th>
                    <th>Move</th>
                    <th>Status</th>
                    <th>Result</th>
                    <th></th>
                  </tr>
                </thead>
                <tbody>
                  ${games
                    .map(
                      (g) => html`
                        <tr>
                          <td class="mono">${g.game_id.slice(0, 8)}…</td>
                          <td>${g.turn}</td>
                          <td>${g.fullmove_number}</td>
                          <td>
                            <span
                              class="badge ${g.is_over
                                ? 'badge-dim'
                                : 'badge-ok'}"
                              >${g.is_over ? 'Over' : 'Active'}</span
                            >
                          </td>
                          <td>${resultLabel(g.result)}</td>
                          <td class="btn-row">
                            <button
                              class="btn btn-sm"
                              data-action="open-game"
                              data-id="${g.game_id}"
                            >
                              Open
                            </button>
                            <button
                              class="btn btn-sm btn-danger"
                              data-action="delete-game"
                              data-id="${g.game_id}"
                            >
                              Delete
                            </button>
                          </td>
                        </tr>
                      `
                    )
                    .join('')}
                </tbody>
              </table>
            </div>`}
      </div>
    `;
  },
});

// ── Component: Board view ───────────────────────────────────────────────────

component('cai-board', {
  styles: `:host { display:block; }`,
  render() {
    const g = activeGame.value;
    if (!g)
      return html`<div class="card empty-card">
        <h2>No game selected</h2>
        <p class="dim">
          Open or create a game from the dashboard or games list.
        </p>
      </div>`;

    const boardHtml = renderBoard(
      g.state.board,
      boardFlipped.value,
      !g.is_over
    );
    const history = g.move_history;
    const currentFen = `${
      Object.entries(g.state.board).filter(([, piece]) => Boolean(piece)).length
    } pieces on board`;

    return html`
      <div class="board-layout">
        <div class="board-main">
          <div class="card board-card">
            <div class="card-head">
              <div>
                <h2>Game <span class="mono">${g.game_id.slice(0, 8)}</span></h2>
                <p class="dim">
                  ${g.is_over
                    ? `Game over — ${resultLabel(g.result)}${g.end_reason ? ' (' + g.end_reason + ')' : ''}`
                    : `${g.state.turn}'s turn · Move ${g.state.fullmove_number}${g.is_check ? ' · Check!' : ''}`}
                </p>
              </div>
              <div class="btn-row">
                <button class="btn btn-sm" data-action="flip-board">
                  ${boardFlipped.value ? '⟳ Reset' : '⟳ Flip'}
                </button>
                <button class="btn btn-sm" data-action="refresh-game">
                  Refresh
                </button>
                <button class="btn btn-sm" data-action="save-pgn">
                  Save PGN
                </button>
              </div>
            </div>
            <div class="board-container" data-board-interactive>
              ${boardHtml}
            </div>
            ${!g.is_over
              ? html`
                  <div class="action-bar">
                    <button class="btn btn-ghost btn-sm" data-action="resign">
                      🏳 Resign
                    </button>
                    <button
                      class="btn btn-ghost btn-sm"
                      data-action="offer-draw"
                    >
                      ½ Offer draw
                    </button>
                    <button
                      class="btn btn-ghost btn-sm"
                      data-action="claim-draw-threefold"
                    >
                      3× Threefold
                    </button>
                    <button
                      class="btn btn-ghost btn-sm"
                      data-action="claim-draw-fifty"
                    >
                      50 Fifty-move
                    </button>
                  </div>
                `
              : html`
                  <div class="action-bar">
                    <button
                      class="btn btn-primary btn-sm"
                      data-action="analyze-game"
                      data-id="${g.game_id}"
                    >
                      📊 Analyze
                    </button>
                    <button class="btn btn-sm" data-action="create-game">
                      ➕ New game
                    </button>
                  </div>
                `}
          </div>
        </div>
        <div class="board-sidebar">
          <div class="card">
            <div class="card-head">
              <div><h3>ℹ Info</h3></div>
            </div>
            <div class="stat-grid">
              <div class="stat">
                <span class="stat-label">Turn</span
                ><strong>${g.state.turn}</strong>
              </div>
              <div class="stat">
                <span class="stat-label">Move</span
                ><strong>${g.state.fullmove_number}</strong>
              </div>
              <div class="stat">
                <span class="stat-label">Legal moves</span
                ><strong>${g.legal_move_count}</strong>
              </div>
              <div class="stat">
                <span class="stat-label">Halfmove clock</span
                ><strong>${g.state.halfmove_clock}</strong>
              </div>
              <div class="stat">
                <span class="stat-label">En passant</span
                ><strong>${g.state.en_passant ?? '—'}</strong>
              </div>
              <div class="stat">
                <span class="stat-label">Castling W</span
                ><strong
                  >${g.state.castling.white.kingside ? 'K' : '-'}${g.state
                    .castling.white.queenside
                    ? 'Q'
                    : '-'}</strong
                >
              </div>
              <div class="stat">
                <span class="stat-label">Castling B</span
                ><strong
                  >${g.state.castling.black.kingside ? 'k' : '-'}${g.state
                    .castling.black.queenside
                    ? 'q'
                    : '-'}</strong
                >
              </div>
            </div>
            <div class="btn-row" style="margin-top:0.75rem">
              <button class="btn btn-sm" data-action="export-fen">
                📋 Copy FEN
              </button>
              <button class="btn btn-sm" data-action="save-fen">
                💾 Save FEN
              </button>
              <button class="btn btn-sm" data-action="export-pgn">
                📋 Copy PGN
              </button>
            </div>
          </div>
          <div class="card move-list-card">
            <div class="card-head">
              <div><h3>📝 Moves</h3></div>
            </div>
            <div class="move-list">
              ${history.length === 0
                ? html`<p class="empty-text">No moves yet.</p>`
                : html`<ol class="moves">
                    ${renderMoveList(history)}
                  </ol>`}
            </div>
          </div>
          <div class="card">
            <div class="card-head">
              <div>
                <h3>🔍 Advanced board view</h3>
                <p class="dim">
                  Desktop-native debugging aids for the current position.
                </p>
              </div>
            </div>
            <div class="stat-grid" style="margin-bottom:0.75rem">
              <div class="stat">
                <span class="stat-label">Game ID</span>
                <strong class="mono">${g.game_id}</strong>
              </div>
              <div class="stat">
                <span class="stat-label">Board state</span>
                <strong>${currentFen}</strong>
              </div>
            </div>
            <pre class="ascii-panel">
${escapeHtml(boardAscii.value || 'Loading ASCII board…')}</pre
            >
            ${desktopState.value.developerMode
              ? html`<details class="details-panel">
                  <summary>Raw game JSON</summary>
                  <pre class="json-panel">
${escapeHtml(JSON.stringify(g, null, 2))}</pre
                  >
                </details>`
              : ''}
          </div>
        </div>
      </div>
    `;
  },
});

function renderMoveList(history: Game['move_history']): string {
  const pairs: string[] = [];
  for (let i = 0; i < history.length; i += 2) {
    const w = history[i];
    const b = history[i + 1];
    pairs.push(
      html`<li>
        <span class="move-num">${w.move_number}.</span>
        <span class="move-w">${safeHtml`${w.notation}`}</span>${b
          ? html` <span class="move-b">${safeHtml`${b.notation}`}</span>`
          : ''}
      </li>`
    );
  }
  return pairs.join('');
}

// ── Component: Archive ──────────────────────────────────────────────────────

component('cai-archive', {
  styles: `:host { display:block; }`,
  render() {
    const rs = replayState.value;
    const list = archivedList.value;

    return html`
      <div class="view-grid">
        ${rs
          ? html`
              <div class="card board-card">
                <div class="card-head">
                  <div>
                    <h2>
                      ▶ Replay ·
                      <span class="mono">${rs.game_id.slice(0, 8)}</span>
                    </h2>
                    <p class="dim">Move ${rs.at_move} / ${rs.total_moves}</p>
                  </div>
                  <div class="btn-row">
                    <button class="btn btn-sm" data-action="replay-start">
                      ⏮
                    </button>
                    <button class="btn btn-sm" data-action="replay-prev">
                      ◀
                    </button>
                    <button class="btn btn-sm" data-action="replay-next">
                      ▶
                    </button>
                    <button class="btn btn-sm" data-action="replay-end">
                      ⏭
                    </button>
                    <button
                      class="btn btn-sm btn-ghost"
                      data-action="close-replay"
                    >
                      ✕ Close
                    </button>
                  </div>
                </div>
                <div class="board-container">
                  ${renderBoard(rs.state.board, boardFlipped.value, false)}
                </div>
                <input
                  type="range"
                  class="replay-slider"
                  min="0"
                  max="${rs.total_moves}"
                  value="${rs.at_move}"
                  data-replay-slider
                />
              </div>
            `
          : ''}
        <div class="card">
          <div class="card-head">
            <div>
              <h2>📦 Archived games</h2>
              <p class="dim">
                ${list.length} completed game${list.length !== 1 ? 's' : ''} in
                the archive
              </p>
            </div>
            <button class="btn btn-ghost btn-sm" data-action="refresh-archive">
              ↻ Refresh
            </button>
          </div>
          ${list.length === 0
            ? html`<div class="empty-card">
                <p class="empty-text">
                  📦 No archived games yet. Complete a game to see it here.
                </p>
              </div>`
            : html`<div class="table-wrap">
                <table class="data-table">
                  <thead>
                    <tr>
                      <th>ID</th>
                      <th>Result</th>
                      <th>Reason</th>
                      <th>Moves</th>
                      <th>Size</th>
                      <th></th>
                    </tr>
                  </thead>
                  <tbody>
                    ${list
                      .map(
                        (g) => html`
                          <tr>
                            <td class="mono">${g.game_id.slice(0, 8)}…</td>
                            <td>${resultLabel(g.result)}</td>
                            <td>${g.end_reason ?? '—'}</td>
                            <td>${g.move_count}</td>
                            <td>${formatBytes(g.compressed_bytes)}</td>
                            <td class="btn-row">
                              <button
                                class="btn btn-sm"
                                data-action="replay-archived"
                                data-id="${g.game_id}"
                              >
                                ▶ Replay
                              </button>
                              <button
                                class="btn btn-sm"
                                data-action="analyze-archived"
                                data-id="${g.game_id}"
                              >
                                📊 Analyze
                              </button>
                            </td>
                          </tr>
                        `
                      )
                      .join('')}
                  </tbody>
                </table>
              </div>`}
        </div>
      </div>
    `;
  },
});

// ── Component: Analysis ─────────────────────────────────────────────────────

component('cai-analysis', {
  styles: `:host { display:block; }`,
  render() {
    const jobs = analysisJobs.value;
    const active = activeAnalysis.value;
    const result = active?.result;
    const summary = result?.summary;

    return html`
      <div class="view-grid">
        ${active
          ? html`
              <div class="card">
                <div class="card-head">
                  <div>
                    <h2>
                      Analysis ·
                      <span class="mono">${active.id.slice(0, 8)}</span>
                    </h2>
                    <p class="dim">${renderAnalysisStatusText(active)}</p>
                  </div>
                  ${analysisPolling.value
                    ? html`<button
                        class="btn btn-ghost btn-sm btn-danger"
                        data-action="cancel-analysis"
                        data-id="${active.id}"
                      >
                        ❌ Cancel
                      </button>`
                    : html`<button
                        class="btn btn-ghost btn-sm"
                        data-action="close-analysis"
                      >
                        ✕ Close
                      </button>`}
                </div>
                ${summary
                  ? html`
                      <div class="analysis-summary">
                        <div class="accuracy-row">
                          <div class="accuracy-block">
                            <span class="accuracy-label">White accuracy</span>
                            <strong class="accuracy-value"
                              >${summary.white_accuracy.toFixed(1)}%</strong
                            >
                            <span class="dim"
                              >avg ±${summary.white_avg_cp_loss.toFixed(1)}
                              cp</span
                            >
                          </div>
                          <div class="accuracy-block">
                            <span class="accuracy-label">Black accuracy</span>
                            <strong class="accuracy-value"
                              >${summary.black_accuracy.toFixed(1)}%</strong
                            >
                            <span class="dim"
                              >avg ±${summary.black_avg_cp_loss.toFixed(1)}
                              cp</span
                            >
                          </div>
                        </div>
                        <div class="quality-bar">
                          ${renderQualityBar(summary)}
                        </div>
                        <div class="stat-grid">
                          <div class="stat">
                            <span class="stat-label">Total moves</span
                            ><strong>${summary.total_moves}</strong>
                          </div>
                          <div class="stat">
                            <span class="stat-label">Best</span
                            ><strong style="color:#34d399"
                              >${summary.best_moves}</strong
                            >
                          </div>
                          <div class="stat">
                            <span class="stat-label">Excellent</span
                            ><strong style="color:#6ee7b7"
                              >${summary.excellent_moves}</strong
                            >
                          </div>
                          <div class="stat">
                            <span class="stat-label">Good</span
                            ><strong style="color:#a3e635"
                              >${summary.good_moves}</strong
                            >
                          </div>
                          <div class="stat">
                            <span class="stat-label">Inaccuracies</span
                            ><strong style="color:#fbbf24"
                              >${summary.inaccuracies}</strong
                            >
                          </div>
                          <div class="stat">
                            <span class="stat-label">Mistakes</span
                            ><strong style="color:#fb923c"
                              >${summary.mistakes}</strong
                            >
                          </div>
                          <div class="stat">
                            <span class="stat-label">Blunders</span
                            ><strong style="color:#fb7185"
                              >${summary.blunders}</strong
                            >
                          </div>
                          <div class="stat">
                            <span class="stat-label">Book moves</span
                            ><strong style="color:#94a3b8"
                              >${summary.book_moves}</strong
                            >
                          </div>
                          <div class="stat">
                            <span class="stat-label">Depth</span
                            ><strong>${result!.depth}</strong>
                          </div>
                          <div class="stat">
                            <span class="stat-label">Avg CP loss</span
                            ><strong
                              >${summary.average_centipawn_loss.toFixed(
                                1
                              )}</strong
                            >
                          </div>
                        </div>
                      </div>
                      ${result!.annotations.length > 0
                        ? html`
                            <div class="annotation-list">
                              <h3>Move annotations</h3>
                              <div class="table-wrap">
                                <table class="data-table compact">
                                  <thead>
                                    <tr>
                                      <th>#</th>
                                      <th>Side</th>
                                      <th>Played</th>
                                      <th>Best</th>
                                      <th>CP loss</th>
                                      <th>Quality</th>
                                      <th>PV</th>
                                    </tr>
                                  </thead>
                                  <tbody>
                                    ${result!.annotations
                                      .map(
                                        (a) => html`
                                          <tr>
                                            <td>${a.move_number}</td>
                                            <td>${a.side}</td>
                                            <td class="mono">
                                              ${a.played_move.from}${a
                                                .played_move.to}${a.played_move
                                                .promotion ?? ''}
                                            </td>
                                            <td class="mono">
                                              ${a.best_move.from}${a.best_move
                                                .to}${a.best_move.promotion ??
                                              ''}
                                            </td>
                                            <td>${a.centipawn_loss}</td>
                                            <td>
                                              <span
                                                class="quality-dot"
                                                style="color:${qualityColor(
                                                  a.quality
                                                )}"
                                                >${safeHtml`${a.quality}`}</span
                                              >
                                            </td>
                                            <td class="mono dim">
                                              ${a.principal_variation
                                                .slice(0, 3)
                                                .join(' ')}
                                            </td>
                                          </tr>
                                        `
                                      )
                                      .join('')}
                                  </tbody>
                                </table>
                              </div>
                            </div>
                          `
                        : ''}
                    `
                  : ''}
              </div>
            `
          : ''}
        <div class="card">
          <div class="card-head">
            <div>
              <h2>Analysis jobs</h2>
              <p class="dim">
                Submit games for deep engine analysis · Configure depth and
                review results.
              </p>
            </div>
            <div class="btn-row">
              <label class="field-inline"
                ><span>Depth:</span
                ><input
                  type="number"
                  id="analysis-depth"
                  min="10"
                  max="99"
                  value="${analysisDepth.value}"
                  style="width:5rem"
              /></label>
              <button
                class="btn btn-ghost btn-sm"
                data-action="refresh-analysis"
              >
                ↻ Refresh
              </button>
            </div>
          </div>
          ${jobs.length === 0
            ? html`<div class="empty-card">
                <p class="empty-text">
                  📊 No analysis jobs yet. Open a game and submit it for
                  analysis.
                </p>
              </div>`
            : html`<div class="table-wrap">
                <table class="data-table">
                  <thead>
                    <tr>
                      <th>Job</th>
                      <th>Game</th>
                      <th>Status</th>
                      <th>Created</th>
                      <th></th>
                    </tr>
                  </thead>
                  <tbody>
                    ${jobs
                      .map(
                        (j) => html`
                          <tr>
                            <td class="mono">${j.id.slice(0, 8)}…</td>
                            <td class="mono">
                              ${j.game_id?.slice(0, 8) ?? '—'}…
                            </td>
                            <td>${renderAnalysisStatusBadge(j)}</td>
                            <td>${formatDateTime(j.created_at)}</td>
                            <td class="btn-row">
                              <button
                                class="btn btn-sm"
                                data-action="view-analysis"
                                data-id="${j.id}"
                              >
                                👁 View
                              </button>
                              ${isAnalysisActive(j)
                                ? html`<button
                                    class="btn btn-sm btn-danger"
                                    data-action="cancel-analysis"
                                    data-id="${j.id}"
                                  >
                                    ❌ Cancel
                                  </button>`
                                : ''}
                            </td>
                          </tr>
                        `
                      )
                      .join('')}
                  </tbody>
                </table>
              </div>`}
        </div>
      </div>
    `;
  },
});

function isAnalysisActive(job: AnalysisJob): boolean {
  return (
    job.status === 'Queued' ||
    (typeof job.status === 'object' && 'InProgress' in job.status)
  );
}

function renderAnalysisStatusText(job: AnalysisJob): string {
  if (job.status === 'Queued') return 'Queued…';
  if (job.status === 'Completed') return 'Completed';
  if (job.status === 'Cancelled') return 'Cancelled';
  if (typeof job.status === 'object') {
    if ('InProgress' in job.status) {
      const p = job.status.InProgress;
      return `Analyzing… ${p.moves_analyzed}/${p.total_moves} moves`;
    }
    if ('Failed' in job.status) return `Failed: ${job.status.Failed.error}`;
  }
  return 'Unknown';
}

function renderAnalysisStatusBadge(job: AnalysisJob): string {
  if (job.status === 'Completed')
    return '<span class="badge badge-ok">Completed</span>';
  if (job.status === 'Queued')
    return '<span class="badge badge-dim">Queued</span>';
  if (job.status === 'Cancelled')
    return '<span class="badge badge-dim">Cancelled</span>';
  if (typeof job.status === 'object' && 'InProgress' in job.status) {
    const p = job.status.InProgress;
    return `<span class="badge badge-active">${p.moves_analyzed}/${p.total_moves}</span>`;
  }
  if (typeof job.status === 'object' && 'Failed' in job.status)
    return '<span class="badge badge-danger">Failed</span>';
  return '<span class="badge badge-dim">?</span>';
}

function renderQualityBar(summary: AnalysisResultPayload['summary']): string {
  const segments = [
    { count: summary.best_moves, color: '#34d399', label: 'Best' },
    { count: summary.excellent_moves, color: '#6ee7b7', label: 'Excellent' },
    { count: summary.good_moves, color: '#a3e635', label: 'Good' },
    { count: summary.inaccuracies, color: '#fbbf24', label: 'Inaccuracy' },
    { count: summary.mistakes, color: '#fb923c', label: 'Mistake' },
    { count: summary.blunders, color: '#fb7185', label: 'Blunder' },
    { count: summary.book_moves, color: '#94a3b8', label: 'Book' },
  ];
  return `<div class="qbar">${segments
    .filter((s) => s.count > 0)
    .map(
      (s) =>
        `<div class="qbar-seg" style="flex:${s.count};background:${s.color}" title="${s.label}: ${s.count}"></div>`
    )
    .join('')}</div>`;
}

// ── Component: Engine config ────────────────────────────────────────────────

component('cai-engine', {
  styles: `:host { display:block; }`,
  render() {
    const ds = desktopState.value;
    const bs = backendStatus.value;
    return html`
      <div class="view-grid">
        <div class="card">
          <div class="card-head">
            <div>
              <h2>⚙ Backend configuration</h2>
              <p class="dim">
                Configure executable, arguments, and working directory for the
                local backend.
              </p>
            </div>
          </div>
          <label class="field"
            ><span>Executable</span>
            <div class="input-with-btn">
              <input
                id="cfg-executable"
                value="${escapeHtml(ds.backendExecutable)}"
                placeholder="checkai"
              /><button
                class="btn btn-ghost btn-sm"
                data-action="pick-executable"
              >
                📁 Browse
              </button>
            </div></label
          >
          <label class="field"
            ><span>Arguments</span
            ><input
              id="cfg-args"
              value="${escapeHtml(ds.backendArgs)}"
              placeholder="serve --analysis-depth 30"
          /></label>
          <label class="field"
            ><span>Working directory</span>
            <div class="input-with-btn">
              <input
                id="cfg-working-dir"
                value="${escapeHtml(ds.backendWorkingDirectory)}"
                placeholder="/path/to/project"
              /><button
                class="btn btn-ghost btn-sm"
                data-action="pick-working-directory"
              >
                📁 Browse
              </button>
            </div></label
          >
          <label class="field"
            ><span>Server URL</span
            ><input
              id="cfg-url"
              value="${escapeHtml(ds.backendUrl)}"
              placeholder="http://127.0.0.1:8080"
          /></label>
          <label class="field checkbox-field"
            ><input
              id="cfg-autostart"
              type="checkbox"
              ${ds.autoStartBackend ? 'checked' : ''}
            /><span>Auto-start backend on launch</span></label
          >
          <div class="btn-row">
            <button class="btn btn-primary btn-sm" data-action="start-backend">
              ▶ Start backend
            </button>
            <button class="btn btn-ghost btn-sm" data-action="stop-backend">
              ⏹ Stop backend
            </button>
            <button class="btn btn-ghost btn-sm" data-action="save-preset">
              🔖 Save preset
            </button>
            <button class="btn btn-ghost btn-sm" data-action="save-settings">
              💾 Save
            </button>
          </div>
          ${bs.lastError
            ? html`<div class="callout callout-danger">
                ${safeHtml`${bs.lastError}`}
              </div>`
            : ''}
        </div>
        <div class="card">
          <div class="card-head">
            <div>
              <h2>📂 Engine assets</h2>
              <p class="dim">
                Opening books and tablebases applied to backend launch.
              </p>
            </div>
          </div>
          <label class="field"
            ><span>Opening book (.bin)</span>
            <div class="input-with-btn">
              <input
                id="cfg-book"
                value="${escapeHtml(ds.openingBookPath)}"
                placeholder="/path/to/book.bin"
              /><button
                class="btn btn-ghost btn-sm"
                data-action="pick-opening-book"
              >
                📁 Browse
              </button>
            </div></label
          >
          <label class="field"
            ><span>Tablebase directory</span>
            <div class="input-with-btn">
              <input
                id="cfg-tablebase"
                value="${escapeHtml(ds.tablebasePath)}"
                placeholder="/path/to/tablebases"
              /><button
                class="btn btn-ghost btn-sm"
                data-action="pick-tablebase"
              >
                📁 Browse
              </button>
            </div></label
          >
          <div class="callout">
            <strong>Note:</strong>
            <span
              >These paths are appended as <code>--book-path</code> and
              <code>--tablebase-path</code> flags unless already present in your
              arguments.</span
            >
          </div>
        </div>
        <div class="card">
          <div class="card-head">
            <div>
              <h2>🗂 Workspace session</h2>
              <p class="dim">Recent folders and persistent launch presets.</p>
            </div>
          </div>
          <div class="stat-grid" style="margin-bottom:0.85rem">
            <div class="stat">
              <span class="stat-label">Current workspace</span>
              <strong>${safeHtml`${currentWorkspace.value}`}</strong>
            </div>
            <div class="stat">
              <span class="stat-label">Recent folders</span>
              <strong>${ds.recentWorkspaces.length}</strong>
            </div>
            <div class="stat">
              <span class="stat-label">Saved presets</span>
              <strong>${ds.backendPresets.length}</strong>
            </div>
          </div>
          ${ds.recentWorkspaces.length > 0
            ? html`<div class="mini-list" style="margin-bottom:0.85rem">
                ${ds.recentWorkspaces
                  .map(
                    (workspace) => html`
                      <button
                        class="mini-item"
                        data-action="use-recent-workspace"
                        data-id="${workspace}"
                      >
                        <span>${safeHtml`${basename(workspace)}`}</span>
                        <span class="mono dim">${safeHtml`${workspace}`}</span>
                      </button>
                    `
                  )
                  .join('')}
              </div>`
            : ''}
          ${ds.backendPresets.length === 0
            ? html`<p class="empty-text">
                No presets yet. Save a profile for recurring runs.
              </p>`
            : html`<div class="preset-list">
                ${ds.backendPresets
                  .map(
                    (preset) => html`
                      <div class="preset-card">
                        <div>
                          <strong>${safeHtml`${preset.name}`}</strong>
                          <p class="dim mono">
                            ${safeHtml`${preset.backendArgs || 'serve'}`}
                          </p>
                        </div>
                        <div class="btn-row">
                          <button
                            class="btn btn-sm"
                            data-action="load-preset"
                            data-id="${preset.id}"
                          >
                            Load
                          </button>
                          <button
                            class="btn btn-sm btn-danger"
                            data-action="delete-preset"
                            data-id="${preset.id}"
                          >
                            Delete
                          </button>
                        </div>
                      </div>
                    `
                  )
                  .join('')}
              </div>`}
        </div>
      </div>
    `;
  },
});

// ── Component: Logs ─────────────────────────────────────────────────────────

component('cai-logs', {
  styles: `:host { display:block; }`,
  render() {
    return html`
      <div class="view-grid logs-grid">
        <div class="card">
          <div class="card-head">
            <div>
              <h2>🩺 Diagnostics</h2>
              <p class="dim">
                Desktop health, live sync, and current runtime context.
              </p>
            </div>
          </div>
          <div class="stat-grid">
            <div class="stat">
              <span class="stat-label">Engine</span>
              <strong
                >${backendStatus.value.running ? 'Running' : 'Stopped'}</strong
              >
            </div>
            <div class="stat">
              <span class="stat-label">Live sync</span>
              <strong>${liveConnection.value}</strong>
            </div>
            <div class="stat">
              <span class="stat-label">Workspace</span>
              <strong>${safeHtml`${currentWorkspace.value}`}</strong>
            </div>
            <div class="stat">
              <span class="stat-label">Last game</span>
              <strong class="mono"
                >${desktopState.value.lastGameId ?? '—'}</strong
              >
            </div>
          </div>
          <div class="callout" style="margin-top:0.85rem">
            <strong>Live status:</strong>
            <span>${safeHtml`${liveMessage.value}`}</span>
          </div>
        </div>
        <div class="card">
          <div class="card-head">
            <div>
              <h2>📜 Backend logs</h2>
              <p class="dim">
                Tail stdout/stderr from the local engine process.
              </p>
            </div>
            <div class="btn-row">
              <button class="btn btn-ghost btn-sm" data-action="refresh-logs">
                ↻ Refresh
              </button>
              <button
                class="btn btn-ghost btn-sm"
                data-action="open-working-dir"
              >
                📂 Open working directory
              </button>
            </div>
          </div>
          <pre class="log-panel">
${escapeHtml(backendLogs.value || 'No logs captured yet.')}</pre
          >
        </div>
        <div class="card">
          <div class="card-head">
            <div>
              <h2>♟ Board ASCII</h2>
              <p class="dim">
                CLI-style board snapshot for debugging and support.
              </p>
            </div>
          </div>
          <pre class="ascii-panel">
${escapeHtml(
              boardAscii.value || 'Open a game to inspect the current board.'
            )}</pre
          >
        </div>
      </div>
    `;
  },
});

// ── Component: Settings ─────────────────────────────────────────────────────

component('cai-settings', {
  styles: `:host { display:block; }`,
  render() {
    const us = updateStatus.value;
    const ds = desktopState.value;
    return html`
      <div class="view-grid">
        <div class="card">
          <div class="card-head">
            <div>
              <h2>⚙ Desktop preferences</h2>
              <p class="dim">Appearance, layout, and notification settings.</p>
            </div>
          </div>
          <div class="stat-grid">
            <div class="stat">
              <span class="stat-label">Theme</span><strong>${ds.theme}</strong>
            </div>
            <div class="stat">
              <span class="stat-label">Board orientation</span
              ><strong
                >${boardFlipped.value
                  ? 'Black at bottom'
                  : 'White at bottom'}</strong
              >
            </div>
            <div class="stat">
              <span class="stat-label">Notifications</span
              ><strong>${ds.notificationsEnabled ? 'Enabled' : 'Muted'}</strong>
            </div>
            <div class="stat">
              <span class="stat-label">Developer mode</span
              ><strong>${ds.developerMode ? 'On' : 'Off'}</strong>
            </div>
            <div class="stat">
              <span class="stat-label">Density</span
              ><strong>${ds.compactMode ? 'Compact' : 'Comfortable'}</strong>
            </div>
          </div>
          <div class="btn-row">
            <button class="btn btn-ghost btn-sm" data-action="toggle-theme">
              🎨 Toggle theme
            </button>
            <button class="btn btn-ghost btn-sm" data-action="flip-board">
              🔄 ${boardFlipped.value ? 'Reset board' : 'Flip board'}
            </button>
            <button
              class="btn btn-ghost btn-sm"
              data-action="toggle-notifications"
            >
              🔔
              ${ds.notificationsEnabled
                ? 'Mute notifications'
                : 'Enable notifications'}
            </button>
            <button
              class="btn btn-ghost btn-sm"
              data-action="toggle-developer-mode"
            >
              🔧 ${ds.developerMode ? 'Hide debug panels' : 'Show debug panels'}
            </button>
            <button
              class="btn btn-ghost btn-sm"
              data-action="toggle-compact-mode"
            >
              📏
              ${ds.compactMode
                ? 'Use comfortable spacing'
                : 'Use compact spacing'}
            </button>
            <button class="btn btn-primary btn-sm" data-action="save-settings">
              💾 Save all settings
            </button>
          </div>
        </div>
        <div class="card">
          <div class="card-head">
            <div>
              <h2>🔄 Desktop updates</h2>
              <p class="dim">Packaged builds can auto-update from GitHub.</p>
            </div>
            <span
              class="badge ${us.state === 'downloaded'
                ? 'badge-ok'
                : 'badge-dim'}"
              >${safeHtml`${us.currentVersion}`}</span
            >
          </div>
          <div class="callout">
            <strong>Status:</strong>
            <span>${safeHtml`${us.message ?? 'Ready.'}`}</span>
          </div>
          ${us.percent !== null
            ? html`
                <div class="progress-bar">
                  <div
                    class="progress-fill"
                    style="width:${Math.round(us.percent)}%"
                  ></div>
                </div>
                <p class="dim">
                  ${Math.round(us.percent)}% ·
                  ${formatBytes(us.transferredBytes)} /
                  ${formatBytes(us.totalBytes)}
                </p>
              `
            : ''}
          <div class="btn-row">
            <button class="btn btn-ghost btn-sm" data-action="check-updates">
              🔍 Check for updates
            </button>
            ${us.state === 'available'
              ? html`<button
                  class="btn btn-primary"
                  data-action="download-update"
                >
                  Download
                </button>`
              : ''}
            ${us.state === 'downloaded'
              ? html`<button
                  class="btn btn-primary"
                  data-action="install-update"
                >
                  Install & restart
                </button>`
              : ''}
          </div>
        </div>
        <div class="card">
          <div class="card-head">
            <div>
              <h2>⌨ Keyboard shortcuts</h2>
              <p class="dim">
                Available keyboard shortcuts in the desktop app.
              </p>
            </div>
          </div>
          <ul class="shortcut-list">
            <li><kbd>Ctrl/⌘ + K</kbd> Command palette</li>
            <li><kbd>Ctrl/⌘ + 1–8</kbd> Switch views</li>
            <li><kbd>Ctrl/⌘ + N</kbd> New game</li>
            <li><kbd>Ctrl/⌘ + F</kbd> Flip board</li>
            <li><kbd>Escape</kbd> Close palette / deselect</li>
          </ul>
        </div>
      </div>
    `;
  },
});

// ── Command palette ─────────────────────────────────────────────────────────

component('cai-palette', {
  styles: `:host { display:block; }`,
  render() {
    if (!paletteOpen.value) return '';
    return html`
      <div class="overlay" data-close-palette>
        <div class="palette" role="dialog" aria-modal="true">
          <div class="palette-head">
            <h3>⚡ Quick actions</h3>
            <button class="btn btn-sm btn-ghost" data-close-palette>✕</button>
          </div>
          <label class="field" style="margin-bottom:0.75rem">
            <span>🔍 Search</span>
            <input
              id="palette-query"
              placeholder="Type to filter commands, views, and desktop actions…"
              value="${escapeHtml(paletteQuery.value)}"
            />
          </label>
          <div class="palette-grid">
            ${filteredPaletteActions.value.length === 0
              ? html`<p class="empty-text">No matching actions found.</p>`
              : filteredPaletteActions.value
                  .map(
                    (action) => html`
                      <button class="palette-btn" data-palette="${action.id}">
                        <strong>${safeHtml`${action.label}`}</strong>
                        <span class="dim">${safeHtml`${action.meta}`}</span>
                      </button>
                    `
                  )
                  .join('')}
          </div>
        </div>
      </div>
    `;
  },
});

// ── Root app shell ──────────────────────────────────────────────────────────

const VIEW_LABELS: Record<DesktopView, string> = {
  dashboard: 'Dashboard',
  games: 'Games',
  board: 'Board',
  archive: 'Archive',
  analysis: 'Analysis',
  engine: 'Engine',
  logs: 'Logs',
  settings: 'Settings',
};

const VIEW_ICONS: Record<DesktopView, string> = {
  dashboard: '⌂',
  games: '♟',
  board: '♔',
  archive: '📦',
  analysis: '📊',
  engine: '⚙',
  logs: '📋',
  settings: '🔧',
};

function renderViewContent(view: DesktopView): string {
  switch (view) {
    case 'dashboard':
      return '<cai-dashboard></cai-dashboard>';
    case 'games':
      return '<cai-games></cai-games>';
    case 'board':
      return '<cai-board></cai-board>';
    case 'archive':
      return '<cai-archive></cai-archive>';
    case 'analysis':
      return '<cai-analysis></cai-analysis>';
    case 'engine':
      return '<cai-engine></cai-engine>';
    case 'logs':
      return '<cai-logs></cai-logs>';
    case 'settings':
      return '<cai-settings></cai-settings>';
    default:
      return '<cai-dashboard></cai-dashboard>';
  }
}

function renderShell(): string {
  const view = currentView.value;
  const bs = backendStatus.value;
  const ds = desktopState.value;
  const workspacePath = ds.backendWorkingDirectory.trim();
  const workspaceName = workspacePath
    ? basename(workspacePath)
    : 'No workspace selected';
  const engineState = bs.running
    ? `Running${bs.pid ? ` · PID ${bs.pid}` : ''}`
    : 'Stopped';
  const activeTitle = activeGame.value?.game_id
    ? `Game ${activeGame.value.game_id.slice(0, 8)}`
    : VIEW_LABELS[view];

  return `
    <div class="shell ${ds.compactMode ? 'shell-compact' : ''}">
      <aside class="sidebar">
        <div class="sidebar-top">
          <div class="brand">
            <span class="brand-icon">♔</span>
            <div>
              <h1>CheckAI</h1>
              <p class="dim">Desktop Command Center</p>
            </div>
          </div>
          <div class="sidebar-workspace-card">
            <span class="sidebar-kicker">Workspace</span>
            <strong>${escapeHtml(workspaceName)}</strong>
            <p class="dim mono">${escapeHtml(
              workspacePath ||
                'Select a working directory to unlock local engine workflows.'
            )}</p>
          </div>
        </div>
        <nav class="sidebar-nav">
          <span class="sidebar-section-label">Control center</span>
          ${(Object.keys(VIEW_LABELS) as DesktopView[])
            .map(
              (v) => `
            <button class="nav-btn ${view === v ? 'active' : ''}" data-nav="${v}">
              <span class="nav-icon">${VIEW_ICONS[v]}</span>
              <span>${VIEW_LABELS[v]}</span>
            </button>
          `
            )
            .join('')}
        </nav>
        <div class="sidebar-footer">
          <span class="status-dot ${bs.running ? 'online' : 'offline'}"></span>
          <div>
            <strong>${bs.running ? 'Engine online' : 'Engine offline'}</strong>
            <p class="dim">${escapeHtml(
              bs.running
                ? (bs.command ?? 'Desktop backend session active')
                : 'Ready to launch your local engine workspace'
            )}</p>
          </div>
        </div>
      </aside>
      <main class="content">
        <header class="topbar">
          <div class="topbar-copy">
            <span class="topbar-kicker">${escapeHtml(VIEW_LABELS[view])}</span>
            <h1>${escapeHtml(activeTitle)}</h1>
            <div class="topbar-meta">
              <span class="meta-chip">
                <span class="meta-chip-label">Workspace</span>
                <strong>${escapeHtml(workspaceName)}</strong>
              </span>
              <span class="meta-chip ${bs.running ? 'meta-chip-strong' : ''}">
                <span class="meta-chip-label">Engine</span>
                <strong>${escapeHtml(engineState)}</strong>
              </span>
              <span class="meta-chip">
                <span class="meta-chip-label">Desktop</span>
                <strong>${escapeHtml(updateStatus.value.currentVersion)}</strong>
              </span>
            </div>
          </div>
          <div class="topbar-actions">
            <span class="badge ${liveConnection.value === 'connected' ? 'badge-ok' : 'badge-dim'}">
              ${escapeHtml(liveMessage.value)}
            </span>
            <span class="badge ${ds.notificationsEnabled ? 'badge-ok' : 'badge-dim'}">
              ${ds.notificationsEnabled ? 'Notifications on' : 'Notifications off'}
            </span>
            <button class="btn btn-ghost btn-sm" data-action="save-settings">Save workspace</button>
            <button class="btn btn-primary btn-sm" data-action="toggle-palette">⌘K · Quick actions</button>
          </div>
        </header>
        ${toastMsg.value ? `<div class="toast toast-ok">${escapeHtml(toastMsg.value)}</div>` : ''}
        ${errorMsg.value ? `<div class="toast toast-err">${escapeHtml(errorMsg.value)}</div>` : ''}
        ${renderViewContent(view)}
      </main>
    </div>
    <cai-palette></cai-palette>
  `;
}

// ── Event delegation ────────────────────────────────────────────────────────

const appRoot = document.getElementById('app');
if (!appRoot) throw new Error('Missing #app root element');

appRoot.addEventListener('click', (e) => {
  const target = e.target;
  if (!(target instanceof Element)) return;

  // Close palette
  const closePaletteButton = target.closest<HTMLElement>(
    'button[data-close-palette]'
  );
  if (closePaletteButton) {
    paletteOpen.value = false;
    paletteQuery.value = '';
    return;
  }

  const paletteOverlay = target.closest<HTMLElement>(
    '.overlay[data-close-palette]'
  );
  if (paletteOverlay && target === paletteOverlay) {
    paletteOpen.value = false;
    paletteQuery.value = '';
    return;
  }

  // Square clicks
  const sqEl = target.closest<HTMLElement>('[data-sq]');
  if (sqEl && sqEl.closest('[data-board-interactive]')) {
    void handleSquareClick(sqEl.dataset.sq!);
    return;
  }

  // Navigation
  const navEl = target.closest<HTMLElement>('[data-nav]');
  if (navEl) {
    const v = navEl.dataset.nav as DesktopView;
    currentView.value = v;
    void persistViewSilently();
    if (v === 'games') void refreshGamesList();
    if (v === 'archive') void refreshArchive();
    if (v === 'analysis') void refreshAnalysisJobs();
    if (v === 'logs') void refreshLogs();
    if (v === 'dashboard') {
      void refreshGamesList();
      void apiGetStorageStats()
        .then((s) => (storageStats.value = s))
        .catch(() => {});
    }
    return;
  }

  // Palette actions
  const paletteBtn = target.closest<HTMLElement>('[data-palette]');
  if (paletteBtn) {
    paletteOpen.value = false;
    paletteQuery.value = '';
    const action = paletteBtn.dataset.palette!;
    void handleAction(action);
    return;
  }

  // General actions
  const actionEl = target.closest<HTMLElement>('[data-action]');
  if (actionEl) {
    void handleAction(actionEl.dataset.action!, actionEl.dataset.id);
    return;
  }
});

appRoot.addEventListener('input', (e) => {
  const t = e.target;
  if (!(t instanceof HTMLInputElement)) return;
  switch (t.id) {
    case 'cfg-executable':
      updateDesktopState({ backendExecutable: t.value });
      break;
    case 'cfg-args':
      updateDesktopState({ backendArgs: t.value });
      break;
    case 'cfg-working-dir':
      updateDesktopState({ backendWorkingDirectory: t.value });
      break;
    case 'cfg-url':
      updateDesktopState({ backendUrl: t.value });
      break;
    case 'cfg-book':
      updateDesktopState({ openingBookPath: t.value });
      break;
    case 'cfg-tablebase':
      updateDesktopState({ tablebasePath: t.value });
      break;
    case 'fen-import':
      fenInput.value = t.value;
      break;
    case 'analysis-depth':
      analysisDepth.value = parseInt(t.value) || 30;
      break;
    case 'palette-query':
      paletteQuery.value = t.value;
      break;
  }
});

appRoot.addEventListener('change', (e) => {
  const t = e.target;
  if (!(t instanceof HTMLInputElement)) return;
  if (t.id === 'cfg-autostart')
    updateDesktopState({ autoStartBackend: t.checked });
  if (t.hasAttribute('data-replay-slider')) {
    void replayTo(parseInt(t.value));
  }
});

appRoot.addEventListener('dragover', (e) => {
  const target = e.target;
  if (!(target instanceof Element)) return;
  if (target.closest('[data-drop-zone]')) {
    e.preventDefault();
    importDropActive.value = true;
  }
});

appRoot.addEventListener('dragleave', (e) => {
  const target = e.target;
  if (!(target instanceof Element)) return;
  if (target.closest('[data-drop-zone]')) {
    importDropActive.value = false;
  }
});

appRoot.addEventListener('drop', (e) => {
  const target = e.target;
  if (!(target instanceof Element)) return;
  if (!target.closest('[data-drop-zone]')) return;

  e.preventDefault();
  importDropActive.value = false;
  const file = e.dataTransfer?.files?.[0];
  if (!file) return;
  void file
    .text()
    .then((text) => importFenText(text))
    .catch((err) => showError((err as Error).message));
});

async function handleAction(action: string, id?: string): Promise<void> {
  try {
    if (action.startsWith('nav:')) {
      const v = action.slice(4) as DesktopView;
      currentView.value = v;
      void persistViewSilently();
      if (v === 'games') void refreshGamesList();
      if (v === 'archive') void refreshArchive();
      if (v === 'analysis') void refreshAnalysisJobs();
      if (v === 'logs') void refreshLogs();
      if (v === 'dashboard') {
        void refreshGamesList();
        void apiGetStorageStats()
          .then((s) => (storageStats.value = s))
          .catch(() => {});
      }
      return;
    }
    switch (action) {
      case 'toggle-palette':
        paletteOpen.value = !paletteOpen.value;
        if (!paletteOpen.value) paletteQuery.value = '';
        break;
      case 'create-game':
        await createNewGame();
        break;
      case 'import-fen':
        await importFromFen();
        break;
      case 'import-fen-file':
        await importFenFromFile();
        break;
      case 'open-game':
        if (id) await openGame(id);
        break;
      case 'delete-game':
        if (id) await doDeleteGame(id);
        break;
      case 'refresh-games':
        await refreshGamesList();
        break;
      case 'refresh-game':
        if (activeGame.value) await openGame(activeGame.value.game_id);
        break;
      case 'flip-board':
        updateDesktopState({ boardFlipped: !boardFlipped.value });
        break;
      case 'resign':
        await doAction('resign');
        break;
      case 'offer-draw':
        await doAction('offer_draw');
        break;
      case 'claim-draw-threefold':
        await doAction('claim_draw', 'threefold_repetition');
        break;
      case 'claim-draw-fifty':
        await doAction('claim_draw', 'fifty_move_rule');
        break;
      case 'export-fen':
        await doExportFen();
        break;
      case 'save-fen':
        await doSaveFen();
        break;
      case 'export-pgn':
        await doExportPgn();
        break;
      case 'save-pgn':
        await doSavePgn();
        break;
      case 'refresh-archive':
        await refreshArchive();
        break;
      case 'replay-archived':
        if (id) await openArchivedGame(id);
        break;
      case 'close-replay':
        replayState.value = null;
        replayGameId.value = null;
        break;
      case 'replay-start':
        await replayTo(0);
        break;
      case 'replay-prev':
        if (replayState.value)
          await replayTo(Math.max(0, replayState.value.at_move - 1));
        break;
      case 'replay-next':
        if (replayState.value)
          await replayTo(
            Math.min(
              replayState.value.total_moves,
              replayState.value.at_move + 1
            )
          );
        break;
      case 'replay-end':
        if (replayState.value) await replayTo(replayState.value.total_moves);
        break;
      case 'analyze-game':
      case 'analyze-archived':
        if (id) {
          await submitAnalysis(id);
          currentView.value = 'analysis';
        }
        break;
      case 'refresh-analysis':
        await refreshAnalysisJobs();
        break;
      case 'view-analysis':
        if (id) await pollAnalysisJob(id);
        break;
      case 'cancel-analysis':
        if (id) await cancelAnalysis(id);
        break;
      case 'close-analysis':
        activeAnalysis.value = null;
        break;
      case 'start-backend':
        await startBackend();
        break;
      case 'stop-backend':
        await stopBackend();
        break;
      case 'save-settings':
        await saveDesktopSettings();
        break;
      case 'save-preset':
        saveCurrentPreset();
        break;
      case 'load-preset':
        if (id) loadPresetIntoState(id);
        break;
      case 'delete-preset':
        if (id) removePresetFromState(id);
        break;
      case 'use-recent-workspace':
        if (id) {
          updateDesktopState({ backendWorkingDirectory: id });
          updateRecentWorkspaces(id);
          showToast(`Workspace “${basename(id)}” selected`);
        }
        break;
      case 'refresh-logs':
        await refreshLogs();
        break;
      case 'open-working-dir': {
        const p = desktopState.value.backendWorkingDirectory.trim();
        if (p) await desktop.openPath(p);
        break;
      }
      case 'pick-executable': {
        const f = await desktop.pickFile();
        if (f) updateDesktopState({ backendExecutable: f });
        break;
      }
      case 'pick-working-directory': {
        const d = await desktop.pickDirectory();
        if (d) updateDesktopState({ backendWorkingDirectory: d });
        break;
      }
      case 'pick-opening-book': {
        const f = await desktop.pickFile();
        if (f) updateDesktopState({ openingBookPath: f });
        break;
      }
      case 'pick-tablebase': {
        const d = await desktop.pickDirectory();
        if (d) updateDesktopState({ tablebasePath: d });
        break;
      }
      case 'check-updates':
        updateStatus.value = await desktop.checkForUpdates();
        break;
      case 'download-update':
        updateStatus.value = await desktop.downloadUpdate();
        break;
      case 'install-update':
        await desktop.installUpdate();
        break;
      case 'toggle-theme': {
        const next =
          desktopState.value.theme === 'dark'
            ? 'light'
            : desktopState.value.theme === 'light'
              ? 'system'
              : 'dark';
        updateDesktopState({ theme: next });
        syncTheme(next);
        break;
      }
      case 'toggle-notifications':
        updateDesktopState({
          notificationsEnabled: !desktopState.value.notificationsEnabled,
        });
        break;
      case 'toggle-developer-mode':
        updateDesktopState({
          developerMode: !desktopState.value.developerMode,
        });
        break;
      case 'toggle-compact-mode':
        updateDesktopState({ compactMode: !desktopState.value.compactMode });
        break;
    }
  } catch (err) {
    showError((err as Error).message);
  }
}

// ── Keyboard shortcuts ──────────────────────────────────────────────────────

window.addEventListener('keydown', (e) => {
  const mod = e.metaKey || e.ctrlKey;
  if (mod && e.key.toLowerCase() === 'k') {
    e.preventDefault();
    paletteOpen.value = true;
    paletteQuery.value = '';
    return;
  }
  if (mod && e.key.toLowerCase() === 'n') {
    e.preventDefault();
    void createNewGame();
    return;
  }
  if (mod && e.key.toLowerCase() === 'f') {
    e.preventDefault();
    updateDesktopState({ boardFlipped: !boardFlipped.value });
    return;
  }
  if (mod && /^[1-8]$/.test(e.key)) {
    e.preventDefault();
    const views: DesktopView[] = [
      'dashboard',
      'games',
      'board',
      'archive',
      'analysis',
      'engine',
      'logs',
      'settings',
    ];
    currentView.value = views[parseInt(e.key) - 1];
    void persistViewSilently();
    return;
  }
  if (e.key === 'Escape') {
    if (paletteOpen.value) {
      paletteOpen.value = false;
      paletteQuery.value = '';
      return;
    }
    selectedSquare.value = null;
  }
  if (e.key === 'Enter' && paletteOpen.value) {
    const first = filteredPaletteActions.value[0];
    if (!first) return;
    e.preventDefault();
    paletteOpen.value = false;
    paletteQuery.value = '';
    void handleAction(first.id);
  }
});

// ── Reactive rendering ──────────────────────────────────────────────────────

let pendingFrame: number | null = null;
let pendingMarkup = '';

effect(() => {
  pendingMarkup = renderShell();
  if (pendingFrame !== null) return;
  pendingFrame = requestAnimationFrame(() => {
    pendingFrame = null;
    // Save focused input
    const ae = document.activeElement;
    let focusId = '';
    let selStart: number | null = null;
    let selEnd: number | null = null;
    if (ae instanceof HTMLInputElement && ae.id) {
      focusId = ae.id;
      selStart = ae.selectionStart;
      selEnd = ae.selectionEnd;
    }
    const tpl = document.createElement('template');
    tpl.innerHTML = pendingMarkup;
    appRoot.replaceChildren(tpl.content);
    // Restore focus
    if (focusId) {
      const el = appRoot.querySelector<HTMLInputElement>(
        `#${CSS.escape(focusId)}`
      );
      if (el) {
        el.focus({ preventScroll: true });
        if (selStart !== null && selEnd !== null)
          el.setSelectionRange(selStart, selEnd);
      }
    }
  });
});

// ── Initialize ──────────────────────────────────────────────────────────────

async function init(): Promise<void> {
  desktopState.value = await desktop.getState();
  currentView.value = desktopState.value.lastView;
  backendStatus.value = await desktop.getBackendStatus();
  backendLogs.value = await desktop.getBackendLogs();
  updateStatus.value = await desktop.getUpdateStatus();
  setApiBase(desktopState.value.backendUrl);
  syncTheme(desktopState.value.theme);
  connectLiveUpdates();

  if (desktopState.value.lastGameId) {
    void refreshBoardAscii(desktopState.value.lastGameId).catch(() => {});
  }

  desktop.onBackendStatus((s) => {
    backendStatus.value = s;
  });
  desktop.onBackendLogs((l) => {
    backendLogs.value = l;
  });
  desktop.onUpdateStatus((s) => {
    updateStatus.value = s;
  });

  // Initial data load
  void refreshGamesList().catch(() => {});
  void apiGetStorageStats()
    .then((s) => (storageStats.value = s))
    .catch(() => {});
  void refreshArchive().catch(() => {});
  void refreshAnalysisJobs().catch(() => {});

  if (desktopState.value.lastGameId) {
    void openGame(
      desktopState.value.lastGameId,
      currentView.value === 'board'
    ).catch(() => {});
  }
}

void init();
