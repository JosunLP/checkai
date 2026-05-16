const BACKEND_URL_EXAMPLE = 'http://127.0.0.1:8080';
const URL_WITH_SCHEME_PATTERN = /^[a-z][a-z0-9+.-]*:\/\//i;
const LOOPBACK_HOSTS = new Set(['127.0.0.1', 'localhost', '::1', '[::1]']);
export const DEFAULT_BACKEND_PORT = '8080';

export function normalizeBackendUrl(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) {
    throw new Error(`Enter a backend URL such as ${BACKEND_URL_EXAMPLE}.`);
  }

  const candidate = URL_WITH_SCHEME_PATTERN.test(trimmed) ? trimmed : `http://${trimmed}`;

  let url: URL;
  try {
    url = new URL(candidate);
  } catch {
    throw new Error(`Enter a valid backend URL such as ${BACKEND_URL_EXAMPLE}.`);
  }

  if (url.protocol !== 'http:') {
    throw new Error('Backend URL must start with http://.');
  }

  if (!url.hostname) {
    throw new Error('Backend URL must include a hostname.');
  }

  if (!LOOPBACK_HOSTS.has(url.hostname.toLowerCase())) {
    throw new Error(
      'Backend URL must use a local loopback host such as 127.0.0.1, localhost, or [::1].'
    );
  }

  url.username = '';
  url.password = '';
  url.search = '';
  url.hash = '';
  if (!url.port) {
    url.port = DEFAULT_BACKEND_PORT;
  }
  url.pathname = url.pathname === '/' ? '' : url.pathname.replace(/\/+$/, '');

  return `${url.origin}${url.pathname}`;
}

export function normalizeBackendUrlOrFallback(value: unknown, fallback: string): string {
  const candidate = typeof value === 'string' ? value : fallback;

  try {
    return normalizeBackendUrl(candidate);
  } catch {
    return fallback;
  }
}
