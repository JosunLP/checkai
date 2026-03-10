export const DESKTOP_VIEWS = ['workspace', 'live', 'engine', 'logs', 'help'] as const;

export type DesktopView = (typeof DESKTOP_VIEWS)[number];

export interface DesktopState {
  backendUrl: string;
  autoStartBackend: boolean;
  backendExecutable: string;
  backendArgs: string;
  backendWorkingDirectory: string;
  openingBookPath: string;
  tablebasePath: string;
  lastView: DesktopView;
}

export interface BackendStatusPayload {
  running: boolean;
  pid: number | null;
  command: string | null;
  startedAt: number | null;
  exitCode: number | null;
  lastError: string | null;
}

export interface UpdateStatusPayload {
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

export interface DesktopApi {
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

export const DEFAULT_DESKTOP_STATE: DesktopState = {
  backendUrl: 'http://127.0.0.1:8080',
  autoStartBackend: false,
  backendExecutable: 'checkai',
  backendArgs: 'serve',
  backendWorkingDirectory: '',
  openingBookPath: '',
  tablebasePath: '',
  lastView: 'workspace',
};
