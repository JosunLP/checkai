import { pushError } from '../notifications.js';

function formatError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export async function attemptRefresh(
  silent: boolean,
  task: () => Promise<void>
): Promise<boolean> {
  try {
    await task();
    return true;
  } catch (error) {
    if (!silent) {
      pushError(formatError(error));
    }
    return false;
  }
}

export async function refreshStore<T>(
  silent: boolean,
  load: () => Promise<T>,
  apply: (value: T) => void
): Promise<boolean> {
  return attemptRefresh(silent, async () => {
    apply(await load());
  });
}
