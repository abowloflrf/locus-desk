<script lang="ts">
  import type { SessionInfo } from '../api/types';
  import TaskBoard from '../components/TaskBoard.svelte';

  let { session, refreshToken = 0 }: { session: SessionInfo; refreshToken?: number } = $props();
</script>

<div class="page tasks-page">
  <TaskBoard mode="all" {refreshToken} today={session.workspace.today} />
</div>

<style>
  .tasks-page {
    width: min(100%, 920px);
    padding-top: 32px;
  }
  @media (max-width: 767px) {
    .tasks-page {
      padding-top: 16px;
    }
  }
</style>
