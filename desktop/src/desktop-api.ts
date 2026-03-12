import { get } from 'svelte/store';
import {
  desktopState,
  currentView,
  backendStatus,
  backendLogs,
  updateStatus,
} from './stores.js';
import { pushError, pushToast } from './notifications.js';
import {
  DEFAULT_DESKTOP_STATE,
  type DesktopApi,
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

export const desktop = window.checkaiDesktop ?? fallbackApi;

// Helper functions
export async function loadDesktopState(): Promise<void> {
  try {
    const state = await desktop.getState();
    desktopState.set(state);
    currentView.set(state.lastView);
  } catch (error) {
    console.error('Failed to load desktop state:', error);
    pushError('Failed to load desktop state.');
  }
}

export async function saveDesktopState(): Promise<void> {
  try {
    const state = get(desktopState);
    await desktop.saveState(state);
  } catch (error) {
    console.error('Failed to save desktop state:', error);
    pushError('Failed to save desktop state.');
  }
}

export function initializeBackendListener(): () => void {
  const unsubscribeStatus = desktop.onBackendStatus((status) => {
    backendStatus.set(status);
  });

  const unsubscribeLogs = desktop.onBackendLogs((logs) => {
    backendLogs.set(logs);
  });

  return () => {
    unsubscribeStatus();
    unsubscribeLogs();
  };
}

export function initializeUpdateListener(): () => void {
  const unsubscribeUpdate = desktop.onUpdateStatus((status) => {
    updateStatus.set(status);
  });

  return () => {
    unsubscribeUpdate();
  };
}

export async function startBackend(): Promise<void> {
  try {
    const state = get(desktopState);
    const status = await desktop.startBackend(state);
    backendStatus.set(status);
    if (status.running) {
      pushToast('Backend started successfully');
    }
  } catch (error) {
    console.error('Failed to start backend:', error);
    pushError('Failed to start backend.');
  }
}

export async function stopBackend(): Promise<void> {
  try {
    const status = await desktop.stopBackend();
    backendStatus.set(status);
    pushToast('Backend stopped');
  } catch (error) {
    console.error('Failed to stop backend:', error);
    pushError('Failed to stop backend.');
  }
}
