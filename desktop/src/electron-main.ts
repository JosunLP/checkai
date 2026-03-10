import { app, BrowserWindow, dialog, ipcMain, Notification, shell } from 'electron';
import { autoUpdater } from 'electron-updater';
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join, resolve } from 'node:path';
import {
  DEFAULT_DESKTOP_STATE,
  DESKTOP_VIEWS,
  type BackendStatusPayload,
  type DesktopState,
  type UpdateStatusPayload,
} from './shared-types.js';

const MAX_LOG_LINES = 400;
const LOG_PUSH_DELAY_MS = 250;

let mainWindow: BrowserWindow | null = null;
let backendProcess: ChildProcessWithoutNullStreams | null = null;
let backendExitListener:
  | ((code: number | null, signal: NodeJS.Signals | null) => void)
  | null = null;
let backendLogs = '';
let backendLogsFlushTimer: NodeJS.Timeout | null = null;
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

function normalizeDesktopState(value: unknown): DesktopState {
  const candidate = typeof value === 'object' && value !== null ? value : {};
  const record = candidate as Record<string, unknown>;
  const lastView = normalizeString(record.lastView, DEFAULT_DESKTOP_STATE.lastView);
  const normalizedLastView =
    DESKTOP_VIEWS.find((view) => view === lastView) ?? DEFAULT_DESKTOP_STATE.lastView;

  return {
    backendUrl: normalizeString(record.backendUrl, DEFAULT_DESKTOP_STATE.backendUrl),
    autoStartBackend:
      typeof record.autoStartBackend === 'boolean'
        ? record.autoStartBackend
        : DEFAULT_DESKTOP_STATE.autoStartBackend,
    backendExecutable: normalizeString(
      record.backendExecutable,
      DEFAULT_DESKTOP_STATE.backendExecutable,
    ),
    backendArgs: normalizeString(record.backendArgs, DEFAULT_DESKTOP_STATE.backendArgs),
    backendWorkingDirectory: normalizeString(record.backendWorkingDirectory),
    openingBookPath: normalizeString(record.openingBookPath),
    tablebasePath: normalizeString(record.tablebasePath),
    lastView: normalizedLastView,
  };
}

function loadState(): DesktopState {
  const file = stateFilePath();
  if (!existsSync(file)) {
    return { ...DEFAULT_DESKTOP_STATE };
  }

  try {
    return normalizeDesktopState(JSON.parse(readFileSync(file, 'utf8')) as Partial<DesktopState>);
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

function buildBackendArgs(state: DesktopState): string[] {
  const args = splitArgs(state.backendArgs);
  if (state.openingBookPath && !args.includes('--book-path')) {
    args.push('--book-path', state.openingBookPath);
  }
  if (state.tablebasePath && !args.includes('--tablebase-path')) {
    args.push('--tablebase-path', state.tablebasePath);
  }

  try {
    const url = new URL(state.backendUrl);
    if (/^\d+$/.test(url.port) && !args.includes('--port')) {
      args.push('--port', url.port);
    }
  } catch {
    // Ignore invalid URLs here; the renderer validates what it stores.
  }

  return args;
}

function notify(title: string, body: string): void {
  if (!Notification.isSupported()) return;
  new Notification({ title, body }).show();
}

function validateOpenPathTarget(target: unknown): string {
  const value = normalizeString(target).trim();
  if (!value) {
    throw new Error('Select a local path first.');
  }

  const looksLikeWindowsDrivePath = /^[a-zA-Z]:[\\/]/.test(value);
  if (!looksLikeWindowsDrivePath && /^[a-zA-Z][a-zA-Z\d+\-.]*:/.test(value)) {
    throw new Error('Only local filesystem paths can be opened from the desktop shell.');
  }

  if (!existsSync(value)) {
    throw new Error('The selected path does not exist.');
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
      percent: 0,
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

function startBackend(state: DesktopState): BackendStatusPayload {
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

  backendExitListener = (code) => {
    if (backendProcess !== processRef) {
      return;
    }

    backendProcess = null;
    backendExitListener = null;
    backendStatus = {
      running: false,
      pid: null,
      command,
      startedAt,
      exitCode: code,
      lastError: code === 0 ? null : `Backend exited with code ${code ?? -1}.`,
    };
    flushBackendLogs();
    pushBackendStatus();
    notify('CheckAI Desktop', 'Local backend stopped.');
  };
  processRef.on('exit', backendExitListener);

  return backendStatus;
}

function stopBackend(): BackendStatusPayload {
  if (!backendProcess) {
    return backendStatus;
  }

  const processRef = backendProcess;
  if (backendExitListener) {
    processRef.removeListener('exit', backendExitListener);
    backendExitListener = null;
  }

  try {
    processRef.kill();
  } catch (error) {
    backendStatus = {
      ...backendStatus,
      running: false,
      pid: null,
      lastError: error instanceof Error ? error.message : String(error),
    };
    pushBackendStatus();
    return backendStatus;
  }

  backendProcess = null;
  backendStatus = {
    ...backendStatus,
    running: false,
    pid: null,
    lastError: null,
  };
  flushBackendLogs();
  pushBackendStatus();
  notify('CheckAI Desktop', 'Local backend stopped.');
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
  return result.filePaths[0] ?? null;
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

  mainWindow.on('closed', () => {
    mainWindow = null;
  });
}

app.whenReady().then(() => {
  configureAutoUpdater();
  ipcMain.handle('checkai:get-state', () => loadState());
  ipcMain.handle('checkai:save-state', (_event, state: unknown) => saveState(state));
  ipcMain.handle('checkai:get-backend-status', () => backendStatus);
  ipcMain.handle('checkai:get-backend-logs', () => backendLogs);
  ipcMain.handle('checkai:get-update-status', () => updateStatus);
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
  ipcMain.handle('checkai:open-path', async (_event, target: unknown) => {
    const path = validateOpenPathTarget(target);
    const result = await shell.openPath(path);
    if (result) {
      throw new Error(result);
    }
  });
  ipcMain.handle('checkai:open-external', (_event, target: unknown) =>
    shell.openExternal(validateExternalTarget(target)),
  );
  ipcMain.handle('checkai:notify', (_event, title: string, body: string) => notify(title, body));

  createWindow();

  const state = loadState();
  if (state.autoStartBackend) {
    startBackend(state);
  }

  void checkForUpdates().catch((error) => {
    console.error('Failed to check for desktop updates at startup:', error);
    notify(
      'CheckAI Desktop',
      'Automatic desktop update check failed. Open Help and retry the update check.',
    );
  });

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
