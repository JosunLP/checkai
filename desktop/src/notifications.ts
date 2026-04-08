import { errorMsg, toastMsg } from './stores.js';

const TOAST_DURATION_MS = 3200;
const ERROR_DURATION_MS = 5000;

let toastTimer: ReturnType<typeof setTimeout> | null = null;
let errorTimer: ReturnType<typeof setTimeout> | null = null;

export function clearNotificationTimers(): void {
  if (toastTimer) {
    clearTimeout(toastTimer);
    toastTimer = null;
  }

  if (errorTimer) {
    clearTimeout(errorTimer);
    errorTimer = null;
  }
}

export function pushToast(message: string): void {
  toastMsg.set(message);
  if (toastTimer) {
    clearTimeout(toastTimer);
  }
  toastTimer = setTimeout(() => {
    toastMsg.set(null);
    toastTimer = null;
  }, TOAST_DURATION_MS);
}

export function pushError(message: string): void {
  errorMsg.set(message);
  if (errorTimer) {
    clearTimeout(errorTimer);
  }
  errorTimer = setTimeout(() => {
    errorMsg.set(null);
    errorTimer = null;
  }, ERROR_DURATION_MS);
}
