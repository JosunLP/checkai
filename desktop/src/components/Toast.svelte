<script lang="ts">
  export let message: string;
  export let type: 'ok' | 'error' = 'ok';
</script>

<div class="toast toast-{type === 'ok' ? 'ok' : 'err'}">
  {message}
</div>
