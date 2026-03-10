import { contextBridge, ipcRenderer } from 'electron';
import type {
  BackendStatusPayload,
  DesktopApi,
  DesktopState,
  UpdateStatusPayload,
} from './shared-types.js';

const api: DesktopApi = {
  getState: (): Promise<DesktopState> => ipcRenderer.invoke('checkai:get-state'),
  saveState: (state: DesktopState): Promise<DesktopState> => ipcRenderer.invoke('checkai:save-state', state),
  getBackendStatus: (): Promise<BackendStatusPayload> => ipcRenderer.invoke('checkai:get-backend-status'),
  getBackendLogs: (): Promise<string> => ipcRenderer.invoke('checkai:get-backend-logs'),
  getUpdateStatus: (): Promise<UpdateStatusPayload> => ipcRenderer.invoke('checkai:get-update-status'),
  startBackend: (state: DesktopState): Promise<BackendStatusPayload> =>
    ipcRenderer.invoke('checkai:start-backend', state),
  stopBackend: (): Promise<BackendStatusPayload> => ipcRenderer.invoke('checkai:stop-backend'),
  checkForUpdates: (): Promise<UpdateStatusPayload> => ipcRenderer.invoke('checkai:check-for-updates'),
  downloadUpdate: (): Promise<UpdateStatusPayload> => ipcRenderer.invoke('checkai:download-update'),
  installUpdate: (): Promise<void> => ipcRenderer.invoke('checkai:install-update'),
  pickFile: (): Promise<string | null> => ipcRenderer.invoke('checkai:pick-file'),
  pickDirectory: (): Promise<string | null> => ipcRenderer.invoke('checkai:pick-directory'),
  openPath: (path: string): Promise<void> => ipcRenderer.invoke('checkai:open-path', path),
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
  onUpdateStatus: (callback: (status: UpdateStatusPayload) => void): (() => void) => {
    const listener = (_event: Electron.IpcRendererEvent, payload: UpdateStatusPayload) => {
      callback(payload);
    };
    ipcRenderer.on('checkai:update-status', listener);
    return () => ipcRenderer.removeListener('checkai:update-status', listener);
  },
};

contextBridge.exposeInMainWorld('checkaiDesktop', api);
