<script lang="ts">
  import { onMount } from 'svelte';
  import { toastMsg, errorMsg } from '../stores.js';

  export let message: string;
  export let type: 'ok' | 'error' = 'ok';

  onMount(() => {
    const timer = setTimeout(() => {
      if (type === 'ok') {
        toastMsg.set(null);
      } else {
        errorMsg.set(null);
      }
    }, 5000);

    return () => clearTimeout(timer);
  });
</script>

<div class="toast toast-{type === 'ok' ? 'ok' : 'err'}">
  {message}
</div>
