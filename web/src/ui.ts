// ============================================================================
// CheckAI Web UI — UI Helpers
// ============================================================================

/** Set text content of an element by ID. */
export function setText(id: string, value: string | number): void {
  const el = document.getElementById(id);
  if (el) el.textContent = String(value);
}

/** Format bytes into human-readable string. */
export function formatBytes(bytes: number | null | undefined): string {
  if (!bytes) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

/** Display a transient toast notification. */
export function showToast(
  message: string,
  type: 'info' | 'success' | 'warning' | 'error' = 'info'
): void {
  const container = document.getElementById('toast-container');
  if (!container) return;

  const toast = document.createElement('div');
  toast.className = `toast toast-${type}`;
  toast.textContent = message;
  container.appendChild(toast);

  setTimeout(() => {
    toast.classList.add('toast-out');
    setTimeout(() => toast.remove(), 250);
  }, 4000);
}

/** Display a game-view inline message. */
export function showGameMessage(
  text: string,
  type: 'info' | 'success' | 'warning' | 'error' = 'info'
): void {
  const el = document.getElementById('game-message');
  if (!el) return;
  el.textContent = text;
  el.className = `game-message msg-${type}`;
  el.style.display = 'block';
  setTimeout(() => {
    el.style.display = 'none';
  }, 6000);
}
