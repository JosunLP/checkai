const BACKEND_URL_EXAMPLE = 'http://127.0.0.1:8080';
const URL_PROTOCOL_PATTERN = /^[a-z][a-z0-9+.-]*:/i;

export function normalizeBackendUrl(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) {
    throw new Error(`Enter a backend URL such as ${BACKEND_URL_EXAMPLE}.`);
  }

  const candidate = URL_PROTOCOL_PATTERN.test(trimmed) ? trimmed : `http://${trimmed}`;

  let url: URL;
  try {
    url = new URL(candidate);
  } catch {
    throw new Error(`Enter a valid backend URL such as ${BACKEND_URL_EXAMPLE}.`);
  }

  if (url.protocol !== 'http:' && url.protocol !== 'https:') {
    throw new Error('Backend URL must start with http:// or https://.');
  }

  if (!url.hostname) {
    throw new Error('Backend URL must include a hostname.');
  }

  url.username = '';
  url.password = '';
  url.search = '';
  url.hash = '';
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
