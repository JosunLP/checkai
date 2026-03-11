<script lang="ts">
  import { backendLogs } from '../stores.js';
</script>

<div class="card">
  <div class="card-head">
    <h2>Backend Logs</h2>
  </div>

  <pre class="log-panel">{$backendLogs || 'No logs available'}</pre>
</div>
