import {
  app,
  BrowserWindow,
  dialog,
  ipcMain,
  Menu,
  Notification,
  shell,
  type MenuItemConstructorOptions,
} from 'electron';
import { autoUpdater } from 'electron-updater';
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  DEFAULT_DESKTOP_STATE,
  DESKTOP_VIEWS,
  type BackendPreset,
  type BackendStatusPayload,
  type DesktopState,
  type SaveTextFileOptions,
  type UpdateStatusPayload,
} from './shared-types.js';
import { DEFAULT_BACKEND_PORT, normalizeBackendUrlOrFallback } from './backend-url.js';

const MAX_LOG_LINES = 400;
const LOG_PUSH_DELAY_MS = 250;
const BACKEND_FORCE_KILL_TIMEOUT_MS = 10_000;
const DEFAULT_NOTIFICATION_TITLE = 'CheckAI Desktop';
// Keep this in sync with CLI flags that consume the following argv entry as a value
// before the subcommand appears, e.g. `checkai --lang de serve`.
const CLI_FLAGS_WITH_SEPARATE_VALUE = new Set(['--lang', '-l']);

let mainWindow: BrowserWindow | null = null;
let backendProcess: ChildProcessWithoutNullStreams | null = null;
let backendExitListener:
  | ((code: number | null, signal: NodeJS.Signals | null) => void)
  | null = null;
let backendStopRequested = false;
let backendLogs = '';
let backendLogsFlushTimer: NodeJS.Timeout | null = null;
let backendForceKillTimer: NodeJS.Timeout | null = null;
const readableFileSelections = new Set<string>();
let backendStatus: BackendStatusPayload = {
  running: false,
  pid: null,
  command: null,
  startedAt: null,
  exitCode: null,
  lastError: null,
};
let updateStatus: UpdateStatusPayload = {
  supported: app.isPackaged,
  currentVersion: app.getVersion(),
  state: app.isPackaged ? 'idle' : 'unsupported',
  availableVersion: null,
  percent: null,
  transferredBytes: null,
  totalBytes: null,
  message: app.isPackaged
    ? 'Ready to check for desktop updates.'
    : 'Desktop updates are available in packaged builds.',
};

function stateFilePath(): string {
  return join(app.getPath('userData'), 'desktop-state.json');
}

function normalizeString(value: unknown, fallback = ''): string {
  return typeof value === 'string' ? value : fallback;
}

function normalizeTheme(value: unknown): 'dark' | 'light' | 'system' {
  if (value === 'dark' || value === 'light' || value === 'system') return value;
  return DEFAULT_DESKTOP_STATE.theme;
}

function normalizePreset(value: unknown): BackendPreset | null {
  const candidate = typeof value === 'object' && value !== null ? value : null;
  if (!candidate) return null;

  const record = candidate as Record<string, unknown>;
  const id = normalizeString(record.id).trim();
  const name = normalizeString(record.name).trim();
  if (!id || !name) return null;

  return {
    id,
    name,
    backendExecutable: normalizeString(record.backendExecutable),
    backendArgs: normalizeString(record.backendArgs),
    backendWorkingDirectory: normalizeString(record.backendWorkingDirectory),
    backendUrl: normalizeBackendUrlOrFallback(
      record.backendUrl,
      DEFAULT_DESKTOP_STATE.backendUrl
    ),
    openingBookPath: normalizeString(record.openingBookPath),
    tablebasePath: normalizeString(record.tablebasePath),
    autoStartBackend:
      typeof record.autoStartBackend === 'boolean'
        ? record.autoStartBackend
        : DEFAULT_DESKTOP_STATE.autoStartBackend,
    createdAt:
      typeof record.createdAt === 'number' && Number.isFinite(record.createdAt)
        ? record.createdAt
        : Date.now(),
  };
}

function normalizeStringArray(value: unknown, limit = 8): string[] {
  if (!Array.isArray(value)) return [];
  const seen = new Set<string>();
  const normalized: string[] = [];

  for (const entry of value) {
    const str = normalizeString(entry).trim();
    if (!str || seen.has(str)) continue;
    seen.add(str);
    normalized.push(str);
    if (normalized.length >= limit) break;
  }

  return normalized;
}

function normalizeDesktopState(value: unknown): DesktopState {
  const candidate = typeof value === 'object' && value !== null ? value : {};
  const record = candidate as Record<string, unknown>;
  const lastView = normalizeString(
    record.lastView,
    DEFAULT_DESKTOP_STATE.lastView
  );
  const normalizedLastView =
    DESKTOP_VIEWS.find((view) => view === lastView) ??
    DEFAULT_DESKTOP_STATE.lastView;

  return {
    backendUrl: normalizeBackendUrlOrFallback(
      record.backendUrl,
      DEFAULT_DESKTOP_STATE.backendUrl
    ),
    autoStartBackend:
      typeof record.autoStartBackend === 'boolean'
        ? record.autoStartBackend
        : DEFAULT_DESKTOP_STATE.autoStartBackend,
    backendExecutable: normalizeString(
      record.backendExecutable,
      DEFAULT_DESKTOP_STATE.backendExecutable
    ),
    backendArgs: normalizeString(
      record.backendArgs,
      DEFAULT_DESKTOP_STATE.backendArgs
    ),
    backendWorkingDirectory: normalizeString(record.backendWorkingDirectory),
    openingBookPath: normalizeString(record.openingBookPath),
    tablebasePath: normalizeString(record.tablebasePath),
    lastView: normalizedLastView,
    theme: normalizeTheme(record.theme),
    boardFlipped:
      typeof record.boardFlipped === 'boolean'
        ? record.boardFlipped
        : DEFAULT_DESKTOP_STATE.boardFlipped,
    recentWorkspaces: normalizeStringArray(record.recentWorkspaces, 10),
    backendPresets: Array.isArray(record.backendPresets)
      ? record.backendPresets
          .map((preset) => normalizePreset(preset))
          .filter((preset): preset is BackendPreset => preset !== null)
          .slice(0, 20)
      : DEFAULT_DESKTOP_STATE.backendPresets,
    notificationsEnabled:
      typeof record.notificationsEnabled === 'boolean'
        ? record.notificationsEnabled
        : DEFAULT_DESKTOP_STATE.notificationsEnabled,
    compactMode:
      typeof record.compactMode === 'boolean'
        ? record.compactMode
        : DEFAULT_DESKTOP_STATE.compactMode,
    developerMode:
      typeof record.developerMode === 'boolean'
        ? record.developerMode
        : DEFAULT_DESKTOP_STATE.developerMode,
    lastGameId:
      record.lastGameId === null
        ? null
        : normalizeString(record.lastGameId) ||
          DEFAULT_DESKTOP_STATE.lastGameId,
  };
}

function loadState(): DesktopState {
  const file = stateFilePath();
  if (!existsSync(file)) {
    return { ...DEFAULT_DESKTOP_STATE };
  }

  try {
    return normalizeDesktopState(
      JSON.parse(readFileSync(file, 'utf8')) as Partial<DesktopState>
    );
  } catch {
    return { ...DEFAULT_DESKTOP_STATE };
  }
}

function saveState(next: unknown): DesktopState {
  const sanitized = normalizeDesktopState(next);
  const file = stateFilePath();
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, JSON.stringify(sanitized, null, 2));
  return sanitized;
}

function pushBackendStatus(): void {
  mainWindow?.webContents.send('checkai:backend-status', backendStatus);
}

function flushBackendLogs(): void {
  if (backendLogsFlushTimer) {
    clearTimeout(backendLogsFlushTimer);
    backendLogsFlushTimer = null;
  }
  mainWindow?.webContents.send('checkai:backend-logs', backendLogs);
}

function scheduleBackendLogsPush(): void {
  if (backendLogsFlushTimer) {
    return;
  }

  backendLogsFlushTimer = setTimeout(() => {
    backendLogsFlushTimer = null;
    mainWindow?.webContents.send('checkai:backend-logs', backendLogs);
  }, LOG_PUSH_DELAY_MS);
}

function appendBackendLogs(chunk: string): void {
  const combined = `${backendLogs}${chunk}`;
  const lines = combined.split(/\r?\n/);
  backendLogs = lines.slice(-MAX_LOG_LINES).join('\n');
  scheduleBackendLogsPush();
}

function pushUpdateStatus(): void {
  mainWindow?.webContents.send('checkai:update-status', updateStatus);
}

function splitArgs(value: string): string[] {
  const matches = value.match(/"[^"]*"|'[^']*'|\S+/g);
  return (matches ?? []).map((part) => part.replace(/^['"]|['"]$/g, ''));
}

function defaultBackendPort(): string {
  return DEFAULT_BACKEND_PORT;
}

function hasCliFlag(args: string[], flag: string): boolean {
  return args.some((arg) => arg === flag || arg.startsWith(`${flag}=`));
}

function findSubcommandIndex(args: string[]): number {
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (!arg.startsWith('-')) {
      return index;
    }

    if (CLI_FLAGS_WITH_SEPARATE_VALUE.has(arg)) {
      index += 1;
    }
  }

  return -1;
}

function buildBackendArgs(state: DesktopState): string[] {
  const args = splitArgs(state.backendArgs);

  let subcommand: string;
  const subcommandIndex = findSubcommandIndex(args);
  if (subcommandIndex === -1) {
    subcommand = 'serve';
    args.push(subcommand);
  } else {
    subcommand = args[subcommandIndex];
  }

  if (subcommand === 'serve') {
    if (state.openingBookPath && !hasCliFlag(args, '--book-path')) {
      args.push('--book-path', state.openingBookPath);
    }
    if (state.tablebasePath && !hasCliFlag(args, '--tablebase-path')) {
      args.push('--tablebase-path', state.tablebasePath);
    }

    try {
      const url = new URL(state.backendUrl);
      const port = url.port || defaultBackendPort();
      if (port && /^\d+$/.test(port) && !hasCliFlag(args, '--port')) {
        args.push('--port', port);
      }
      if (!hasCliFlag(args, '--host')) {
        args.push('--host', '127.0.0.1');
      }
    } catch {
      // Ignore invalid URLs here; persisted desktop state is normalized before use.
    }
  }

  return args;
}

function clearBackendForceKillTimer(): void {
  if (backendForceKillTimer) {
    clearTimeout(backendForceKillTimer);
    backendForceKillTimer = null;
  }
}

function notify(title: unknown, body: unknown): void {
  const normalizedTitle = validateNotificationTitle(title);
  const normalizedBody = validateNotificationBody(body);
  if (!Notification.isSupported() || !normalizedBody) return;
  new Notification({ title: normalizedTitle, body: normalizedBody }).show();
}

function validateNotificationTitle(value: unknown): string {
  const normalized = normalizeString(value).trim();
  return normalized || DEFAULT_NOTIFICATION_TITLE;
}

function validateNotificationBody(value: unknown): string {
  return normalizeString(value).trim();
}

function getBackendExitError(
  code: number | null,
  signal: NodeJS.Signals | null,
  stopRequested: boolean
): string | null {
  if (signal === 'SIGTERM' || code === 0) {
    return null;
  }

  if (stopRequested) {
    return `Backend exited with code ${code ?? -1} while stopping.`;
  }

  return `Backend exited with code ${code ?? -1}.`;
}

function validateOpenPathTarget(target: unknown): string {
  const value = normalizeString(target).trim();
  if (!value) {
    throw new Error('Select a local path first.');
  }

  const looksLikeWindowsDrivePath = /^[a-zA-Z]:[\\/]/.test(value);
  if (!looksLikeWindowsDrivePath && /^[a-zA-Z][a-zA-Z\d+\-.]*:/.test(value)) {
    throw new Error(
      'Only local filesystem paths can be opened from the desktop shell.'
    );
  }

  const resolvedPath = resolve(value);

  if (!existsSync(resolvedPath)) {
    throw new Error('The selected path does not exist.');
  }

  return resolvedPath;
}

function validateProgressBarValue(value: unknown): number | null {
  if (value === null || value === undefined) {
    return null;
  }

  if (typeof value !== 'number' || Number.isNaN(value) || !Number.isFinite(value)) {
    throw new Error('Progress bar value must be a finite number between 0 and 1.');
  }

  if (value < 0 || value > 1) {
    throw new Error('Progress bar value must be between 0 and 1.');
  }

  return value;
}

function validateExternalTarget(target: unknown): string {
  const value = normalizeString(target).trim();
  let url: URL;

  try {
    url = new URL(value);
  } catch {
    throw new Error('Enter a valid HTTP or HTTPS URL.');
  }

  if (!['http:', 'https:'].includes(url.protocol)) {
    throw new Error('Only HTTP and HTTPS URLs can be opened externally.');
  }

  return url.toString();
}

function configureAutoUpdater(): void {
  if (!app.isPackaged) {
    updateStatus = {
      ...updateStatus,
      supported: false,
      state: 'unsupported',
      message: 'Desktop updates are available in packaged builds.',
    };
    return;
  }

  autoUpdater.autoDownload = false;
  autoUpdater.autoInstallOnAppQuit = true;

  autoUpdater.on('checking-for-update', () => {
    updateStatus = {
      ...updateStatus,
      supported: true,
      currentVersion: app.getVersion(),
      state: 'checking',
      availableVersion: null,
      percent: null,
      transferredBytes: null,
      totalBytes: null,
      message: 'Checking GitHub Releases for a newer desktop build…',
    };
    pushUpdateStatus();
  });

  autoUpdater.on('update-available', (info) => {
    updateStatus = {
      ...updateStatus,
      supported: true,
      state: 'available',
      availableVersion: info.version,
      percent: null,
      transferredBytes: null,
      totalBytes: null,
      message: `Version ${info.version} is available for download.`,
    };
    pushUpdateStatus();
    notify('CheckAI Desktop', `Desktop update ${info.version} is available.`);
  });

  autoUpdater.on('update-not-available', () => {
    updateStatus = {
      ...updateStatus,
      supported: true,
      state: 'up-to-date',
      availableVersion: null,
      percent: null,
      transferredBytes: null,
      totalBytes: null,
      message: `CheckAI Desktop ${app.getVersion()} is up to date.`,
    };
    pushUpdateStatus();
  });

  autoUpdater.on('download-progress', (progress) => {
    updateStatus = {
      ...updateStatus,
      supported: true,
      state: 'downloading',
      percent: progress.percent,
      transferredBytes: progress.transferred,
      totalBytes: progress.total,
      message: `Downloading version ${updateStatus.availableVersion ?? 'update'}…`,
    };
    pushUpdateStatus();
  });

  autoUpdater.on('update-downloaded', (info) => {
    updateStatus = {
      ...updateStatus,
      supported: true,
      state: 'downloaded',
      availableVersion: info.version,
      percent: 100,
      transferredBytes: updateStatus.totalBytes,
      totalBytes: updateStatus.totalBytes,
      message: `Version ${info.version} is ready to install. Restart the app to finish updating.`,
    };
    pushUpdateStatus();
    notify('CheckAI Desktop', `Update ${info.version} is ready to install.`);
  });

  autoUpdater.on('error', (error) => {
    updateStatus = {
      ...updateStatus,
      supported: true,
      state: 'error',
      percent: null,
      transferredBytes: null,
      totalBytes: null,
      message: error instanceof Error ? error.message : String(error),
    };
    pushUpdateStatus();
  });
}

async function checkForUpdates(): Promise<UpdateStatusPayload> {
  if (!app.isPackaged) {
    updateStatus = {
      ...updateStatus,
      supported: false,
      state: 'unsupported',
      message: 'Desktop updates are available in packaged builds.',
    };
    pushUpdateStatus();
    return updateStatus;
  }

  try {
    await autoUpdater.checkForUpdates();
  } catch (error) {
    console.error('Failed to check for desktop updates:', error);
    updateStatus = {
      ...updateStatus,
      supported: true,
      state: 'error',
      message: error instanceof Error ? error.message : String(error),
    };
    pushUpdateStatus();
  }
  return updateStatus;
}

async function downloadUpdate(): Promise<UpdateStatusPayload> {
  if (!app.isPackaged) {
    return checkForUpdates();
  }

  if (updateStatus.state !== 'available') {
    updateStatus = {
      ...updateStatus,
      state: updateStatus.state === 'downloaded' ? 'downloaded' : 'error',
      message:
        updateStatus.state === 'downloaded'
          ? updateStatus.message
          : 'No downloadable desktop update is currently available.',
    };
    pushUpdateStatus();
    return updateStatus;
  }

  updateStatus = {
    ...updateStatus,
    supported: true,
    state: 'downloading',
    percent: 0,
    transferredBytes: 0,
    totalBytes: null,
    message: `Downloading version ${updateStatus.availableVersion ?? 'update'}…`,
  };
  pushUpdateStatus();

  try {
    await autoUpdater.downloadUpdate();
  } catch (error) {
    updateStatus = {
      ...updateStatus,
      supported: true,
      state: 'error',
      percent: null,
      transferredBytes: null,
      totalBytes: null,
      message: error instanceof Error ? error.message : String(error),
    };
    pushUpdateStatus();
  }
  return updateStatus;
}

function installUpdate(): void {
  if (updateStatus.state !== 'downloaded') {
    return;
  }

  autoUpdater.quitAndInstall();
}

function dispatchMenuCommand(command: string): void {
  mainWindow?.webContents.send('checkai:menu-command', command);
}

function buildApplicationMenu(): Menu {
  const template: MenuItemConstructorOptions[] = [
    {
      label: 'File',
      submenu: [
        {
          label: 'New Game',
          accelerator: 'CmdOrCtrl+N',
          click: () => dispatchMenuCommand('new-game'),
        },
        {
          label: 'Import FEN from File…',
          accelerator: 'CmdOrCtrl+O',
          click: () => dispatchMenuCommand('import-fen-file'),
        },
        { type: 'separator' },
        {
          label: 'Copy FEN',
          accelerator: 'CmdOrCtrl+Shift+C',
          click: () => dispatchMenuCommand('export-fen'),
        },
        {
          label: 'Save FEN…',
          accelerator: 'CmdOrCtrl+Shift+S',
          click: () => dispatchMenuCommand('save-fen'),
        },
        {
          label: 'Copy PGN',
          accelerator: 'CmdOrCtrl+Alt+C',
          click: () => dispatchMenuCommand('export-pgn'),
        },
        {
          label: 'Save PGN…',
          accelerator: 'CmdOrCtrl+Alt+S',
          click: () => dispatchMenuCommand('save-pgn'),
        },
        { type: 'separator' },
        process.platform === 'darwin' ? { role: 'close' } : { role: 'quit' },
      ],
    },
    {
      label: 'Navigate',
      submenu: [
        {
          label: 'Dashboard',
          accelerator: 'CmdOrCtrl+1',
          click: () => dispatchMenuCommand('nav:dashboard'),
        },
        {
          label: 'Games',
          accelerator: 'CmdOrCtrl+2',
          click: () => dispatchMenuCommand('nav:games'),
        },
        {
          label: 'Board',
          accelerator: 'CmdOrCtrl+3',
          click: () => dispatchMenuCommand('nav:board'),
        },
        {
          label: 'Archive',
          accelerator: 'CmdOrCtrl+4',
          click: () => dispatchMenuCommand('nav:archive'),
        },
        {
          label: 'Analysis',
          accelerator: 'CmdOrCtrl+5',
          click: () => dispatchMenuCommand('nav:analysis'),
        },
        {
          label: 'Engine',
          accelerator: 'CmdOrCtrl+6',
          click: () => dispatchMenuCommand('nav:engine'),
        },
        {
          label: 'Logs',
          accelerator: 'CmdOrCtrl+7',
          click: () => dispatchMenuCommand('nav:logs'),
        },
        {
          label: 'Settings',
          accelerator: 'CmdOrCtrl+,',
          click: () => dispatchMenuCommand('nav:settings'),
        },
        { type: 'separator' },
        {
          label: 'Command Palette',
          accelerator: 'CmdOrCtrl+K',
          click: () => dispatchMenuCommand('open-command-palette'),
        },
      ],
    },
    {
      label: 'Tools',
      submenu: [
        {
          label: 'Start Backend',
          accelerator: 'CmdOrCtrl+Shift+R',
          click: () => dispatchMenuCommand('start-backend'),
        },
        {
          label: 'Stop Backend',
          accelerator: 'CmdOrCtrl+Shift+.',
          click: () => dispatchMenuCommand('stop-backend'),
        },
        { type: 'separator' },
        {
          label: 'Open Working Directory',
          accelerator: 'CmdOrCtrl+Shift+O',
          click: () => dispatchMenuCommand('open-working-directory'),
        },
        {
          label: 'Check for Updates',
          accelerator: 'CmdOrCtrl+Shift+U',
          click: () => dispatchMenuCommand('check-for-updates'),
        },
        { type: 'separator' },
        { role: 'reload' },
        { role: 'forceReload' },
        { role: 'toggleDevTools' },
      ],
    },
    {
      label: 'Window',
      submenu: [{ role: 'minimize' }, { role: 'zoom' }, { role: 'togglefullscreen' }],
    },
    {
      label: 'Help',
      submenu: [
        {
          label: 'Open CheckAI Documentation',
          click: () => {
            void shell.openExternal('https://github.com/JosunLP/checkai/tree/main/docs');
          },
        },
        {
          label: 'Open Repository',
          click: () => {
            void shell.openExternal('https://github.com/JosunLP/checkai');
          },
        },
      ],
    },
  ];

  return Menu.buildFromTemplate(template);
}

function startBackend(state: DesktopState): BackendStatusPayload {
  if (!backendProcess && backendStopRequested) {
    backendStopRequested = false;
    clearBackendForceKillTimer();
  }

  if (backendProcess && backendStopRequested) {
    backendStatus = {
      ...backendStatus,
      lastError: 'Waiting for the local backend to finish stopping.',
    };
    pushBackendStatus();
    return backendStatus;
  }

  if (backendProcess) {
    return backendStatus;
  }

  const executable = state.backendExecutable.trim();
  if (!executable) {
    backendStatus = {
      ...backendStatus,
      lastError: 'Set a backend executable before starting the local engine.',
    };
    pushBackendStatus();
    return backendStatus;
  }

  const args = buildBackendArgs(state);
  const command = [executable, ...args].join(' ');
  backendLogs = '';
  flushBackendLogs();

  try {
    clearBackendForceKillTimer();
    backendStopRequested = false;
    backendProcess = spawn(executable, args, {
      cwd: state.backendWorkingDirectory.trim() || undefined,
      stdio: 'pipe',
    });
  } catch (error) {
    backendStatus = {
      running: false,
      pid: null,
      command,
      startedAt: null,
      exitCode: null,
      lastError: error instanceof Error ? error.message : String(error),
    };
    flushBackendLogs();
    pushBackendStatus();
    return backendStatus;
  }

  backendStatus = {
    running: true,
    pid: backendProcess.pid ?? null,
    command,
    startedAt: Date.now(),
    exitCode: null,
    lastError: null,
  };
  pushBackendStatus();
  notify('CheckAI Desktop', 'Local backend started.');

  const processRef = backendProcess;
  const startedAt = backendStatus.startedAt;

  processRef.stdout.on('data', (chunk: Buffer) => {
    appendBackendLogs(chunk.toString('utf8'));
  });
  processRef.stderr.on('data', (chunk: Buffer) => {
    appendBackendLogs(chunk.toString('utf8'));
  });
  processRef.on('error', (error) => {
    if (backendProcess !== processRef) {
      return;
    }

    if (backendExitListener) {
      processRef.removeListener('exit', backendExitListener);
      backendExitListener = null;
    }

    clearBackendForceKillTimer();
    backendStopRequested = false;
    backendProcess = null;
    backendStatus = {
      running: false,
      pid: null,
      command,
      startedAt,
      exitCode: null,
      lastError: error instanceof Error ? error.message : String(error),
    };
    flushBackendLogs();
    pushBackendStatus();
  });

  backendExitListener = (
    code: number | null,
    signal: NodeJS.Signals | null
  ) => {
    if (backendProcess !== processRef) {
      return;
    }

    const stopRequested = backendStopRequested;
    clearBackendForceKillTimer();
    backendProcess = null;
    backendExitListener = null;
    backendStatus = {
      running: false,
      pid: null,
      command,
      startedAt,
      exitCode: code,
      lastError: getBackendExitError(code, signal, stopRequested),
    };
    flushBackendLogs();
    pushBackendStatus();
    backendStopRequested = false;
    notify('CheckAI Desktop', 'Local backend stopped.');
  };
  processRef.on('exit', backendExitListener);

  return backendStatus;
}

function stopBackend(): BackendStatusPayload {
  if (!backendProcess) {
    return backendStatus;
  }

  if (backendStopRequested) {
    backendStatus = {
      ...backendStatus,
      lastError: 'Backend stop already in progress.',
    };
    pushBackendStatus();
    return backendStatus;
  }

  const processRef = backendProcess;
  backendStopRequested = true;

  try {
    processRef.kill();
  } catch (error) {
    clearBackendForceKillTimer();
    backendProcess = null;
    backendStopRequested = false;
    backendStatus = {
      ...backendStatus,
      running: false,
      pid: null,
      lastError: error instanceof Error ? error.message : String(error),
    };
    pushBackendStatus();
    return backendStatus;
  }

  clearBackendForceKillTimer();
  backendForceKillTimer = setTimeout(() => {
    if (!backendStopRequested || backendProcess !== processRef) {
      clearBackendForceKillTimer();
      return;
    }

    if (backendExitListener) {
      processRef.removeListener('exit', backendExitListener);
      backendExitListener = null;
    }

    try {
      processRef.kill('SIGKILL');
    } catch {
      // Ignore errors from killing an already-terminated process.
    }

    backendProcess = null;
    backendStopRequested = false;
    backendStatus = {
      ...backendStatus,
      running: false,
      pid: null,
      lastError:
        backendStatus.lastError ??
        'Backend did not exit after the stop request and was force-killed.',
    };
    pushBackendStatus();
    clearBackendForceKillTimer();
  }, BACKEND_FORCE_KILL_TIMEOUT_MS);

  backendStatus = { ...backendStatus, lastError: null };
  pushBackendStatus();
  return backendStatus;
}

async function selectPath(kind: 'file' | 'directory'): Promise<string | null> {
  if (!mainWindow) return null;
  const result = await dialog.showOpenDialog(mainWindow, {
    properties: kind === 'directory' ? ['openDirectory'] : ['openFile'],
  });
  if (result.canceled || result.filePaths.length === 0) {
    return null;
  }
  const selectedPath = result.filePaths[0] ?? null;
  if (!selectedPath) {
    return null;
  }

  const resolvedPath = resolve(selectedPath);
  if (kind === 'file') {
    readableFileSelections.add(resolvedPath);
  }

  return resolvedPath;
}

function validateReadableTextFileTarget(target: unknown): string {
  const path = validateOpenPathTarget(target);
  if (!readableFileSelections.has(path)) {
    throw new Error('Only files selected through the native picker can be read.');
  }

  return path;
}

function readTextFile(target: unknown): string {
  const path = validateReadableTextFileTarget(target);
  const content = readFileSync(path, 'utf8');
  readableFileSelections.delete(path);
  return content;
}

async function saveTextFile(options: unknown): Promise<string | null> {
  if (!mainWindow) return null;

  const candidate =
    typeof options === 'object' && options !== null
      ? (options as Partial<SaveTextFileOptions>)
      : {};
  const defaultPath = normalizeString(candidate.defaultPath).trim();
  const content = normalizeString(candidate.content);
  const filters = Array.isArray(candidate.filters)
    ? candidate.filters
        .map((filter) => {
          const name = normalizeString(filter?.name).trim();
          const extensions = Array.isArray(filter?.extensions)
            ? filter.extensions
                .map((extension) =>
                  normalizeString(extension).trim().replace(/^\./, '')
                )
                .filter(Boolean)
            : [];
          return name && extensions.length > 0 ? { name, extensions } : null;
        })
        .filter(
          (
            filter
          ): filter is {
            name: string;
            extensions: string[];
          } => filter !== null
        )
    : [];

  const result = await dialog.showSaveDialog(mainWindow, {
    defaultPath: defaultPath || undefined,
    filters: filters.length > 0 ? filters : undefined,
  });

  if (result.canceled || !result.filePath) {
    return null;
  }

  writeFileSync(result.filePath, content, 'utf8');
  return result.filePath;
}

function createWindow(): void {
  const __dirname = dirname(fileURLToPath(import.meta.url));
  const preload = join(__dirname, 'preload.js');
  const rendererIndex = resolve(__dirname, '../dist/index.html');

  mainWindow = new BrowserWindow({
    width: 1560,
    height: 980,
    minWidth: 1180,
    minHeight: 760,
    backgroundColor: '#0b1020',
    title: 'CheckAI Desktop',
    webPreferences: {
      preload,
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  void mainWindow.loadFile(rendererIndex);
  Menu.setApplicationMenu(buildApplicationMenu());

  mainWindow.on('closed', () => {
    mainWindow = null;
  });
}

function registerIpcHandlers(): void {
  ipcMain.handle('checkai:get-state', () => loadState());
  ipcMain.handle('checkai:save-state', (_event, state: unknown) =>
    saveState(state)
  );
  ipcMain.handle('checkai:get-backend-status', () => backendStatus);
  ipcMain.handle('checkai:get-backend-logs', () => backendLogs);
  ipcMain.handle('checkai:get-update-status', () => updateStatus);
  ipcMain.handle('checkai:set-progress-bar', (_event, progress: unknown) => {
    const normalized = validateProgressBarValue(progress);
    mainWindow?.setProgressBar(normalized ?? -1);
  });
  ipcMain.handle('checkai:start-backend', (_event, state: unknown) => {
    const saved = saveState(state);
    return startBackend(saved);
  });
  ipcMain.handle('checkai:stop-backend', () => stopBackend());
  ipcMain.handle('checkai:check-for-updates', () => checkForUpdates());
  ipcMain.handle('checkai:download-update', () => downloadUpdate());
  ipcMain.handle('checkai:install-update', () => installUpdate());
  ipcMain.handle('checkai:pick-file', () => selectPath('file'));
  ipcMain.handle('checkai:pick-directory', () => selectPath('directory'));
  ipcMain.handle('checkai:read-text-file', (_event, target: unknown) =>
    readTextFile(target)
  );
  ipcMain.handle('checkai:save-text-file', (_event, options: unknown) =>
    saveTextFile(options)
  );
  ipcMain.handle('checkai:open-path', async (_event, target: unknown) => {
    const path = validateOpenPathTarget(target);
    const result = await shell.openPath(path);
    if (result) {
      throw new Error(result);
    }
  });
  ipcMain.handle('checkai:open-external', (_event, target: unknown) =>
    shell.openExternal(validateExternalTarget(target))
  );
  ipcMain.handle('checkai:notify', (_event, title: unknown, body: unknown) =>
    notify(title, body)
  );
}

registerIpcHandlers();

app.whenReady().then(() => {
  configureAutoUpdater();
  createWindow();

  const state = loadState();
  if (state.autoStartBackend) {
    startBackend(state);
  }

  void checkForUpdates();

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

app.on('before-quit', () => {
  stopBackend();
});

app.on('window-all-closed', () => {
  stopBackend();
  if (process.platform !== 'darwin') {
    app.quit();
  }
});
