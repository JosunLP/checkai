<script lang="ts">
  export let message: string;
  export let type: 'ok' | 'error' = 'ok';
</script>

<div
  class="toast toast-{type === 'ok' ? 'ok' : 'err'}"
  role={type === 'ok' ? 'status' : 'alert'}
  aria-live={type === 'ok' ? 'polite' : 'assertive'}
  aria-atomic="true"
>
  {message}
</div>
