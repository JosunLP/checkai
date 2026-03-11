<script lang="ts">
  import { onDestroy } from 'svelte';
  import { toastMsg, errorMsg } from '../stores.js';

  export let message: string;
  export let type: 'ok' | 'error' = 'ok';

  let timer: ReturnType<typeof setTimeout> | null = null;

  $: {
    if (!message) {
      if (timer !== null) {
        clearTimeout(timer);
        timer = null;
      }
    } else {
      if (timer !== null) {
        clearTimeout(timer);
      }

      const currentType = type;
      timer = setTimeout(() => {
        if (currentType === 'ok') {
          toastMsg.set(null);
        } else if (currentType === 'error') {
          errorMsg.set(null);
        }
        timer = null;
      }, 5000);
    }
  }

  onDestroy(() => {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
  });
</script>

<div class="toast toast-{type === 'ok' ? 'ok' : 'err'}">
  {message}
</div>
