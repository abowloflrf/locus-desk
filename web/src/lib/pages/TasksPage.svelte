<script lang="ts">
  import type { SessionInfo } from '../api/types';
  import TaskBoard from '../components/TaskBoard.svelte';

  let { session, refreshToken = 0 }: { session: SessionInfo; refreshToken?: number } = $props();
</script>

<div class="page tasks-page">
  <header class="page-header">
    <div>
      <p class="eyebrow">Action</p>
      <h1>Tasks</h1>
      <p class="page-description">Plan what matters, then keep the list moving.</p>
    </div>
  </header>

  <TaskBoard mode="all" {refreshToken} today={session.workspace.today} />
</div>
