<script lang="ts">
  import { currentView, desktopState } from '../stores.js';

  const viewTitles: Record<string, string> = {
    dashboard: 'Dashboard',
    games: 'Games',
    board: 'Board',
    archive: 'Archive',
    analysis: 'Analysis',
    engine: 'Engine',
    logs: 'Logs',
    settings: 'Settings',
  };

  $: title = viewTitles[$currentView] || 'CheckAI Desktop';
</script>

<div class="topbar">
  <div class="topbar-copy">
    <span class="topbar-kicker">CheckAI Desktop</span>
    <h1>{title}</h1>
  </div>

  <div class="topbar-actions">
    <!-- Theme toggle and other actions can be added here -->
  </div>
</div>
