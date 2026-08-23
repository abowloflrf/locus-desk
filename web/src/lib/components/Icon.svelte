<script lang="ts">
  export type IconName =
    | 'archive'
    | 'calendar'
    | 'chat'
    | 'close'
    | 'delete'
    | 'edit'
    | 'flag'
    | 'library'
    | 'logout'
    | 'menu'
    | 'note'
    | 'pin'
    | 'plus'
    | 'reader'
    | 'restore'
    | 'search'
    | 'settings'
    | 'tasks'
    | 'today';

  let { name, size = 18 }: { name: IconName; size?: number } = $props();

  const paths: Record<IconName, string[]> = {
    archive: ['M4 7h16', 'M5 7l1 13h12l1-13', 'M3 4h18v3H3z', 'M9 11h6'],
    calendar: ['M7 3v3', 'M17 3v3', 'M4 9h16', 'M6 5h12a2 2 0 0 1 2 2v13H4V7a2 2 0 0 1 2-2z'],
    chat: ['M21 13a8 8 0 0 1-8 8H5l-3 2 1-6a9 9 0 1 1 18-4z'],
    close: ['M5 5l14 14', 'M19 5L5 19'],
    delete: ['M4 7h16', 'M9 7V4h6v3', 'M7 7l1 13h8l1-13', 'M10 11v5', 'M14 11v5'],
    edit: ['M4 20h4L19 9l-4-4L4 16v4z', 'M13.5 6.5l4 4'],
    flag: ['M6 21V4', 'M6 5h10l-2 4 2 4H6'],
    library: ['M4 5h16v15H4z', 'M8 5V3h8v2', 'M9 10h6', 'M9 14h6'],
    logout: ['M10 4H5v16h5', 'M14 8l4 4-4 4', 'M8 12h10'],
    menu: ['M4 7h16', 'M4 12h16', 'M4 17h16'],
    note: ['M5 3h10l4 4v14H5z', 'M14 3v5h5', 'M8 12h8', 'M8 16h6'],
    pin: ['M9 4h6', 'M10 4v5l-3 3v2h10v-2l-3-3V4', 'M12 14v7'],
    plus: ['M12 5v14', 'M5 12h14'],
    reader: ['M3 5c4-1 7 0 9 2v13c-2-2-5-3-9-2z', 'M21 5c-4-1-7 0-9 2v13c2-2 5-3 9-2z'],
    restore: ['M4 12a8 8 0 1 0 3-6', 'M4 4v6h6', 'M12 8v5l3 2'],
    search: ['M11 19a8 8 0 1 1 0-16 8 8 0 0 1 0 16z', 'M17 17l4 4'],
    settings: [
      'M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7z',
      'M19 13.5l2 1-2 3-2-1a8 8 0 0 1-2 1l-.2 2.5h-3.6L11 17.5a8 8 0 0 1-2-1l-2 1-2-3 2-1a8 8 0 0 1 0-3l-2-1 2-3 2 1a8 8 0 0 1 2-1L11.2 4h3.6L15 6.5a8 8 0 0 1 2 1l2-1 2 3-2 1a8 8 0 0 1 0 3z',
    ],
    tasks: ['M9 6h11', 'M9 12h11', 'M9 18h11', 'M4 6l1 1 2-2', 'M4 12l1 1 2-2', 'M4 18l1 1 2-2'],
    today: ['M12 3a9 9 0 1 0 9 9', 'M12 7v5l3 2', 'M4 4l2 2', 'M18 4l-2 2'],
  };
</script>

<svg aria-hidden="true" class="icon" fill="none" height={size} viewBox="0 0 24 24" width={size}>
  {#each paths[name] as path}
    <path d={path}></path>
  {/each}
</svg>

<style>
  .icon {
    display: block;
    flex: none;
    stroke: currentColor;
    stroke-linecap: round;
    stroke-linejoin: round;
    stroke-width: 1.7;
  }
</style>
