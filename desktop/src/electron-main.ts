import { app, BrowserWindow, dialog, ipcMain, Notification, shell } from 'electron';
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join, resolve } from 'node:path';

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

const MAX_LOG_LINES = 400;

let mainWindow: BrowserWindow | null = null;
let backendProcess: ChildProcessWithoutNullStreams | null = null;
let backendLogs = '';
let backendStatus: BackendStatusPayload = {
  running: false,
  pid: null,
  command: null,
  startedAt: null,
  exitCode: null,
  lastError: null,
};

function stateFilePath(): string {
  return join(app.getPath('userData'), 'desktop-state.json');
}

function loadState(): DesktopState {
  const file = stateFilePath();
  if (!existsSync(file)) {
    return { ...DEFAULT_STATE };
  }

  try {
    const parsed = JSON.parse(readFileSync(file, 'utf8')) as Partial<DesktopState>;
    return {
      ...DEFAULT_STATE,
      ...parsed,
    };
  } catch {
    return { ...DEFAULT_STATE };
  }
}

function saveState(next: DesktopState): DesktopState {
  const file = stateFilePath();
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, JSON.stringify(next, null, 2));
  return next;
}

function pushBackendStatus(): void {
  mainWindow?.webContents.send('checkai:backend-status', backendStatus);
}

function appendBackendLogs(chunk: string): void {
  const combined = `${backendLogs}${chunk}`;
  const lines = combined.split(/\r?\n/);
  backendLogs = lines.slice(-MAX_LOG_LINES).join('\n');
  mainWindow?.webContents.send('checkai:backend-logs', backendLogs);
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

  backendProcess.stdout.on('data', (chunk: Buffer) => {
    appendBackendLogs(chunk.toString('utf8'));
  });
  backendProcess.stderr.on('data', (chunk: Buffer) => {
    appendBackendLogs(chunk.toString('utf8'));
  });
  backendProcess.on('exit', (code) => {
    backendProcess = null;
    backendStatus = {
      running: false,
      pid: null,
      command,
      startedAt: backendStatus.startedAt,
      exitCode: code,
      lastError: code === 0 ? null : `Backend exited with code ${code ?? -1}.`,
    };
    pushBackendStatus();
    notify('CheckAI Desktop', 'Local backend stopped.');
  });

  return backendStatus;
}

function stopBackend(): BackendStatusPayload {
  if (backendProcess) {
    backendProcess.kill();
    backendProcess = null;
  }
  backendStatus = {
    ...backendStatus,
    running: false,
    pid: null,
    exitCode: backendStatus.exitCode,
  };
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
  ipcMain.handle('checkai:get-state', () => loadState());
  ipcMain.handle('checkai:save-state', (_event, state: DesktopState) => saveState(state));
  ipcMain.handle('checkai:get-backend-status', () => backendStatus);
  ipcMain.handle('checkai:get-backend-logs', () => backendLogs);
  ipcMain.handle('checkai:start-backend', (_event, state: DesktopState) => {
    const saved = saveState(state);
    return startBackend(saved);
  });
  ipcMain.handle('checkai:stop-backend', () => stopBackend());
  ipcMain.handle('checkai:pick-file', () => selectPath('file'));
  ipcMain.handle('checkai:pick-directory', () => selectPath('directory'));
  ipcMain.handle('checkai:open-path', (_event, target: string) => shell.openPath(target));
  ipcMain.handle('checkai:open-external', (_event, target: string) => shell.openExternal(target));
  ipcMain.handle('checkai:notify', (_event, title: string, body: string) => notify(title, body));

  createWindow();

  const state = loadState();
  if (state.autoStartBackend) {
    startBackend(state);
  }

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    stopBackend();
    app.quit();
  }
});
