<script lang="ts">
  import { onMount } from 'svelte';
  import { modalState } from '../stores.js';

  let primaryInput: HTMLInputElement | null = null;
  let promptValue = '';

  $: if ($modalState?.kind === 'prompt') {
    promptValue = $modalState.initialValue;
  }

  onMount(() => {
    primaryInput?.focus();
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
</script>

{#if $modalState}
  <div class="overlay modal-overlay" role="presentation">
    <div
      class="palette modal-card"
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-labelledby="desktop-modal-title"
      aria-describedby="desktop-modal-message"
      on:keydown={(event) => {
        if (event.key === 'Escape') {
          event.preventDefault();
          if ($modalState.kind === 'confirm') {
            closeConfirm(false);
          } else {
            closePrompt(null);
          }
        }
      }}
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
          <button class="btn btn-ghost" on:click={() => closeConfirm(false)}>
            {$modalState.cancelLabel}
          </button>
          <button class="btn btn-primary" on:click={() => closeConfirm(true)}>
            {$modalState.confirmLabel}
          </button>
        {:else}
          <button class="btn btn-ghost" on:click={() => closePrompt(null)}>
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
