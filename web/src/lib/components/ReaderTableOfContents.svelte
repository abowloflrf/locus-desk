<script lang="ts">
  import ArrowUp from '@lucide/svelte/icons/arrow-up';
  import List from '@lucide/svelte/icons/list';
  import { onMount, tick } from 'svelte';
  import type { ReaderHeading } from '../reader-outline';
  import { Button } from './ui/button';
  import * as Popover from './ui/popover';
  import * as Sheet from './ui/sheet';

  let {
    headings,
    article,
    scrollElement,
    titleElement,
  }: {
    headings: ReaderHeading[];
    article: HTMLElement | undefined;
    scrollElement: HTMLElement | null;
    titleElement: HTMLElement | undefined;
  } = $props();

  let activeId = $state('');
  let previewId = $state<string | null>(null);
  let open = $state(false);
  let compactViewport = $state(false);
  let panel = $state<HTMLDivElement | null>(null);
  let trigger = $state<HTMLButtonElement | null>(null);
  let markers = $state<HTMLElement>();
  let destination: HTMLElement | null = null;
  let returnToTrigger = false;
  let lastPointerType = '';

  function closestHeading(clientY: number) {
    let closestId: string | undefined;
    let distance = Number.POSITIVE_INFINITY;
    for (const marker of markers?.querySelectorAll<HTMLElement>('[data-heading-id]') ?? []) {
      const rect = marker.getBoundingClientRect();
      const nextDistance = Math.abs(clientY - rect.top - rect.height / 2);
      if (nextDistance >= distance) continue;
      distance = nextDistance;
      closestId = marker.dataset.headingId;
    }
    return headings.find((heading) => heading.id === closestId);
  }

  function previewFromRuler(event: PointerEvent) {
    if (event.pointerType === 'touch' || compactViewport) return;
    previewId = closestHeading(event.clientY)?.id ?? null;
  }

  function followFromRuler(event: MouseEvent) {
    if (event.detail === 0 || lastPointerType === 'touch') return;
    const heading = closestHeading(event.clientY);
    if (heading) follow(event, heading);
  }

  $effect(() => {
    if (!open) previewId = null;
  });

  onMount(() => {
    const query = window.matchMedia('(max-width: 767px), (hover: none)');
    const update = () => {
      compactViewport = query.matches;
      open = false;
    };
    update();
    query.addEventListener('change', update);
    return () => query.removeEventListener('change', update);
  });

  function focusCurrent(event: Event) {
    if (event.defaultPrevented) return;
    event.preventDefault();
    destination = null;
    const link =
      panel?.querySelector<HTMLElement>('a[aria-current="location"]') ??
      panel?.querySelector<HTMLElement>('a');
    link?.focus({ preventScroll: true });
  }

  function restoreFocus(event: Event) {
    if (destination) {
      event.preventDefault();
      destination.focus({ preventScroll: true });
      destination = null;
    } else if (returnToTrigger) {
      event.preventDefault();
      trigger?.focus({ preventScroll: true });
    }
    returnToTrigger = false;
  }

  $effect(() => {
    const entries = headings;
    const body = article;
    const root = scrollElement;
    if (!body) return;
    let disposed = false;
    let frame = 0;
    let observer: ResizeObserver | undefined;
    const target = root ?? window;

    function update() {
      frame = 0;
      const top = Math.max(0, root?.getBoundingClientRect().top ?? 0);
      const height = root?.clientHeight || window.innerHeight;
      const activationLine = top + Math.min(160, height * 0.25);
      let current = entries[0]?.id ?? '';
      for (const heading of body!.querySelectorAll<HTMLElement>('h2[id], h3[id]')) {
        if (heading.getBoundingClientRect().top > activationLine) break;
        current = heading.id;
      }
      if (
        root &&
        root.scrollTop > 0 &&
        root.scrollTop + root.clientHeight >= root.scrollHeight - 2
      ) {
        current = entries.at(-1)?.id ?? current;
      }
      activeId = current;
    }

    function schedule() {
      if (!frame) frame = requestAnimationFrame(update);
    }

    void tick().then(() => {
      if (disposed) return;
      update();
      target.addEventListener('scroll', schedule, { passive: true });
      window.addEventListener('resize', schedule);
      if (typeof ResizeObserver !== 'undefined') {
        observer = new ResizeObserver(schedule);
        observer.observe(body);
        if (root) observer.observe(root);
      }
    });

    return () => {
      disposed = true;
      cancelAnimationFrame(frame);
      observer?.disconnect();
      target.removeEventListener('scroll', schedule);
      window.removeEventListener('resize', schedule);
    };
  });

  $effect(() => {
    const id = previewId ?? activeId;
    const container = panel;
    const link = container?.querySelector<HTMLElement>(`a[href="#${id}"]`);
    if (!container || !link) return;
    const bounds = container.getBoundingClientRect();
    const item = link.getBoundingClientRect();
    if (item.top < bounds.top + 16 || item.bottom > bounds.bottom - 16) {
      container.scrollTop += item.top - bounds.top - bounds.height / 2 + item.height / 2;
    }
  });

  function navigate(element: HTMLElement | null | undefined) {
    if (!element) return;
    destination = element;
    previewId = null;
    open = false;
    element.focus({ preventScroll: true });
    const offset = window.matchMedia('(min-width: 768px) and (max-width: 1199px)').matches
      ? 128
      : 80;
    const top =
      element === titleElement
        ? 0
        : Math.max(
            0,
            (scrollElement?.scrollTop ?? window.scrollY) +
              element.getBoundingClientRect().top -
              (scrollElement?.getBoundingClientRect().top ?? 0) -
              offset,
          );
    const behavior = window.matchMedia('(prefers-reduced-motion: reduce)').matches
      ? 'instant'
      : 'smooth';
    (scrollElement ?? window).scrollTo({ top, behavior });
  }

  function follow(event: MouseEvent, heading: ReaderHeading) {
    event.preventDefault();
    activeId = heading.id;
    navigate(document.getElementById(heading.id));
  }
</script>

{#snippet triggerContents()}
  <span class="toc-mobile"><List size={18} />Contents</span>
  <span
    class="toc-markers"
    class:previewing={previewId !== null}
    style={`--toc-count: ${headings.length}`}
    aria-hidden="true"
    bind:this={markers}
  >
    {#each headings as heading (heading.id)}
      <span
        data-heading-id={heading.id}
        class:subsection={heading.depth === 3}
        class:current={activeId === heading.id}
        class:previewed={previewId === heading.id}
      ></span>
    {/each}
  </span>
{/snippet}

{#snippet contents()}
  <nav aria-label="Table of contents">
    <div class="toc-heading">
      <span>On this page</span>
      <Button
        aria-label="Back to top"
        onclick={() => navigate(titleElement)}
        size="icon"
        variant="ghost"
      >
        <ArrowUp />
      </Button>
    </div>
    <ol class="toc-list" class:previewing={previewId !== null}>
      {#each headings as heading (heading.id)}
        <li>
          <a
            href={`#${heading.id}`}
            aria-current={activeId === heading.id ? 'location' : undefined}
            class:subsection={heading.depth === 3}
            data-preview-current={previewId === heading.id ? '' : undefined}
            onpointerenter={(event) => {
              if (event.pointerType !== 'touch' && !compactViewport) previewId = heading.id;
            }}
            onclick={(event) => follow(event, heading)}>{heading.text}</a
          >
        </li>
      {/each}
    </ol>
  </nav>
{/snippet}

<aside class="reader-toc" aria-label="Article navigation">
  <div class="toc-peek">
    {#if compactViewport}
      <Sheet.Root bind:open>
        <Sheet.Trigger
          bind:ref={trigger}
          aria-label="Table of contents"
          class="toc-trigger"
          title="Table of contents"
        >
          {@render triggerContents()}
        </Sheet.Trigger>
        <Sheet.Content
          bind:ref={panel}
          class="w-[min(22rem,calc(100vw-2rem))] overflow-y-auto overscroll-contain p-4 pt-14"
          onOpenAutoFocus={focusCurrent}
          onCloseAutoFocus={restoreFocus}
          onEscapeKeydown={() => (returnToTrigger = true)}
        >
          <Sheet.Title class="sr-only">Table of contents</Sheet.Title>
          <Sheet.Description class="sr-only">Navigate article sections.</Sheet.Description>
          {@render contents()}
        </Sheet.Content>
      </Sheet.Root>
    {:else}
      <Popover.Root bind:open>
        <Popover.Trigger
          bind:ref={trigger}
          aria-label="Table of contents"
          class="toc-trigger"
          title="Table of contents"
          openOnHover
          openDelay={0}
          closeDelay={160}
          onpointerenter={previewFromRuler}
          onpointermove={previewFromRuler}
          onpointerdown={(event) => (lastPointerType = event.pointerType)}
          onclick={followFromRuler}
          onkeydown={() => (previewId = null)}
        >
          {@render triggerContents()}
        </Popover.Trigger>
        <Popover.Content
          bind:ref={panel}
          aria-label="Table of contents"
          side="left"
          align="start"
          sideOffset={8}
          class="max-h-[min(70dvh,36rem)] w-[min(20rem,calc(100vw-2rem))] overflow-y-auto overscroll-contain"
          onOpenAutoFocus={focusCurrent}
          onCloseAutoFocus={restoreFocus}
          onEscapeKeydown={() => (returnToTrigger = true)}
          onkeydown={() => (previewId = null)}
        >
          {@render contents()}
        </Popover.Content>
      </Popover.Root>
    {/if}
  </div>
</aside>

<style>
  .reader-toc {
    position: sticky;
    top: 96px;
    grid-column: 2;
    grid-row: 1;
    align-self: start;
    min-width: 0;
  }

  .reader-toc :global(.toc-trigger) {
    display: flex;
    width: 44px;
    min-height: 44px;
    justify-content: center;
    padding: 12px 8px;
    border: 0;
    border-radius: 8px;
    background: var(--background);
    color: var(--muted-foreground);
    cursor: pointer;
    transition: background-color 160ms ease;
  }
  .reader-toc :global(.toc-trigger:hover),
  .reader-toc :global(.toc-trigger[aria-expanded='true']) {
    background: var(--muted);
  }
  .reader-toc :global(.toc-trigger:focus-visible),
  .toc-list a:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  .toc-markers {
    display: grid;
    grid-template-rows: repeat(var(--toc-count), minmax(0, 1fr));
    align-items: center;
    justify-items: end;
    width: 28px;
    height: min(calc(var(--toc-count) * 8px), 40dvh);
    overflow: hidden;
  }
  .toc-markers span {
    width: 24px;
    height: 2px;
    max-height: 100%;
    background: currentColor;
    transition:
      width 160ms ease,
      background-color 160ms ease;
  }
  .toc-markers .subsection {
    width: 16px;
  }
  .toc-markers:not(.previewing) .current,
  .toc-markers .previewed {
    width: 28px;
    height: 3px;
    background: var(--primary);
  }
  .toc-mobile {
    display: none;
  }
  .toc-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-bottom: 8px;
  }
  .toc-heading span {
    font-size: 12px;
    font-weight: 650;
    color: var(--muted-foreground);
  }
  .toc-list {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .toc-list a {
    display: flex;
    align-items: center;
    min-height: 36px;
    padding: 8px 12px;
    border-inline-start: 2px solid transparent;
    color: var(--muted-foreground);
    font-size: 13px;
    line-height: 1.45;
    text-decoration: none;
    overflow-wrap: anywhere;
    transition: color 160ms ease;
  }
  .toc-list a.subsection {
    padding-inline-start: 24px;
    font-size: 12px;
  }
  .toc-list a:hover {
    color: var(--foreground);
  }
  .toc-list:not(.previewing) a[aria-current='location'],
  .toc-list a[data-preview-current] {
    color: var(--primary);
    border-color: var(--primary);
    font-weight: 650;
  }

  @media (min-width: 768px) and (max-width: 1199px) {
    .reader-toc {
      top: 144px;
    }
  }
  @media (max-width: 767px) {
    .reader-toc {
      top: env(safe-area-inset-top, 0px);
      grid-column: 1;
      width: 100%;
      background: var(--background);
      z-index: 9;
    }
    .toc-peek {
      display: flex;
      justify-content: end;
    }
    .reader-toc :global(.toc-trigger) {
      width: auto;
    }
    .toc-mobile {
      display: flex;
      gap: 8px;
      align-items: center;
      font-size: 13px;
    }
    .toc-markers {
      display: none;
    }
  }
  @media (max-width: 767px), (pointer: coarse) {
    .toc-list a {
      min-height: 44px;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .reader-toc :global(.toc-trigger),
    .toc-markers span,
    .toc-list a {
      transition: none;
    }
  }
</style>
