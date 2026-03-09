import { contextBridge, ipcRenderer } from 'electron';

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

const api = {
  getState: (): Promise<DesktopState> => ipcRenderer.invoke('checkai:get-state'),
  saveState: (state: DesktopState): Promise<DesktopState> => ipcRenderer.invoke('checkai:save-state', state),
  getBackendStatus: (): Promise<BackendStatusPayload> => ipcRenderer.invoke('checkai:get-backend-status'),
  getBackendLogs: (): Promise<string> => ipcRenderer.invoke('checkai:get-backend-logs'),
  startBackend: (state: DesktopState): Promise<BackendStatusPayload> =>
    ipcRenderer.invoke('checkai:start-backend', state),
  stopBackend: (): Promise<BackendStatusPayload> => ipcRenderer.invoke('checkai:stop-backend'),
  pickFile: (): Promise<string | null> => ipcRenderer.invoke('checkai:pick-file'),
  pickDirectory: (): Promise<string | null> => ipcRenderer.invoke('checkai:pick-directory'),
  openPath: (path: string): Promise<string> => ipcRenderer.invoke('checkai:open-path', path),
  openExternal: (url: string): Promise<void> => ipcRenderer.invoke('checkai:open-external', url),
  notify: (title: string, body: string): Promise<void> => ipcRenderer.invoke('checkai:notify', title, body),
  onBackendStatus: (callback: (status: BackendStatusPayload) => void): (() => void) => {
    const listener = (_event: Electron.IpcRendererEvent, payload: BackendStatusPayload) => {
      callback(payload);
    };
    ipcRenderer.on('checkai:backend-status', listener);
    return () => ipcRenderer.removeListener('checkai:backend-status', listener);
  },
  onBackendLogs: (callback: (logs: string) => void): (() => void) => {
    const listener = (_event: Electron.IpcRendererEvent, payload: string) => {
      callback(payload);
    };
    ipcRenderer.on('checkai:backend-logs', listener);
    return () => ipcRenderer.removeListener('checkai:backend-logs', listener);
  },
};

contextBridge.exposeInMainWorld('checkaiDesktop', api);
