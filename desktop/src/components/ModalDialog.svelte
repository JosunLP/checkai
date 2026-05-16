<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { trapTabKey } from '../accessibility.js';
  import { modalState } from '../stores.js';

  let dialogElement: HTMLDivElement | null = null;
  let initialActionButton: HTMLButtonElement | null = null;
  let primaryInput: HTMLInputElement | null = null;
  let promptValue = '';
  let restoreFocusElement: HTMLElement | null = null;

  $: if ($modalState?.kind === 'prompt') {
    promptValue = $modalState.initialValue;
  }

  function canRestoreFocus(element: HTMLElement): boolean {
    return (
      document.contains(element) &&
      (!('disabled' in element) || !(element as HTMLButtonElement | HTMLInputElement).disabled) &&
      element.getAttribute('aria-hidden') !== 'true' &&
      element.getClientRects().length > 0
    );
  }

  async function focusActiveControl(): Promise<void> {
    await tick();

    if ($modalState?.kind === 'prompt') {
      primaryInput?.focus();
      return;
    }

    if (initialActionButton) {
      initialActionButton.focus();
    } else {
      dialogElement?.focus();
    }
  }

  onMount(() => {
    restoreFocusElement = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    void focusActiveControl();

    return () => {
      if (restoreFocusElement && canRestoreFocus(restoreFocusElement)) {
        restoreFocusElement.focus();
      }
    };
  });

  function closeConfirm(result: boolean): void {
    const state = $modalState;
    if (!state || state.kind !== 'confirm') {
      return;
    }
    modalState.set(null);
    state.resolve(result);
  }

  function closePrompt(result: string | null): void {
    const state = $modalState;
    if (!state || state.kind !== 'prompt') {
      return;
    }
    modalState.set(null);
    state.resolve(result);
  }

  function handleWindowKeydown(event: KeyboardEvent): void {
    if (!$modalState) {
      return;
    }

    if (event.key === 'Tab') {
      trapTabKey(event, dialogElement);
      return;
    }

    if (event.key === 'Escape') {
      event.preventDefault();
      if ($modalState?.kind === 'confirm') {
        closeConfirm(false);
      } else {
        closePrompt(null);
      }
    }
  }
</script>

<svelte:window on:keydown={handleWindowKeydown} />

{#if $modalState}
  <div class="overlay modal-overlay" role="presentation">
    <div
      bind:this={dialogElement}
      class="palette modal-card"
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-labelledby="desktop-modal-title"
      aria-describedby="desktop-modal-message"
    >
      <div class="palette-head">
        <div>
          <h3 id="desktop-modal-title">{$modalState.title}</h3>
          <p id="desktop-modal-message" class="dim">{$modalState.message}</p>
        </div>
      </div>

      {#if $modalState.kind === 'prompt'}
        <label class="field modal-field">
          <span>Value</span>
          <input
            bind:this={primaryInput}
            bind:value={promptValue}
            type="text"
            placeholder={$modalState.placeholder ?? ''}
            on:keydown={(event) => {
              if (event.key === 'Enter' && promptValue.trim()) {
                event.preventDefault();
                closePrompt(promptValue.trim());
              }
            }}
          />
        </label>
      {/if}

      <div class="btn-row modal-actions">
        {#if $modalState.kind === 'confirm'}
          <button
            bind:this={initialActionButton}
            class="btn btn-ghost"
            on:click={() => closeConfirm(false)}
          >
            {$modalState.cancelLabel}
          </button>
          <button class="btn btn-primary" on:click={() => closeConfirm(true)}>
            {$modalState.confirmLabel}
          </button>
        {:else}
          <button
            bind:this={initialActionButton}
            class="btn btn-ghost"
            on:click={() => closePrompt(null)}
          >
            {$modalState.cancelLabel}
          </button>
          <button
            class="btn btn-primary"
            disabled={!promptValue.trim()}
            on:click={() => closePrompt(promptValue.trim())}
          >
            {$modalState.confirmLabel}
          </button>
        {/if}
      </div>
    </div>
  </div>
{/if}
