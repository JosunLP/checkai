const FOCUSABLE_SELECTOR = [
  'button:not([disabled])',
  'a[href]',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(', ');

function isFocusable(element: HTMLElement): boolean {
  const style = window.getComputedStyle(element);
  return (
    element.getAttribute('aria-hidden') !== 'true' &&
    style.visibility !== 'hidden' &&
    style.display !== 'none' &&
    element.offsetWidth > 0 &&
    element.offsetHeight > 0 &&
    element.getClientRects().length > 0
  );
}

export function trapTabKey(event: KeyboardEvent, container: HTMLElement | null): void {
  if (!container) {
    return;
  }

  const focusable = Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    isFocusable
  );

  if (focusable.length === 0) {
    event.preventDefault();
    container.focus();
    return;
  }

  const activeElement = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  const first = focusable[0];
  const last = focusable[focusable.length - 1];
  const focusIsInside = activeElement ? container.contains(activeElement) : false;

  if (event.shiftKey) {
    if (!focusIsInside || activeElement === first) {
      event.preventDefault();
      last.focus();
    }
    return;
  }

  if (!focusIsInside || activeElement === last) {
    event.preventDefault();
    first.focus();
  }
}
