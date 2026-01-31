<script lang="ts">
  import type { LogEntry } from '$lib/types';
  import { _, locale } from 'svelte-i18n';

  interface Props {
    logs: LogEntry[];
  }

  let { logs }: Props = $props();
  let container: HTMLDivElement;
  let autoScroll = $state(true);

  // Auto-scroll to bottom when new logs arrive
  $effect(() => {
    if (logs.length && autoScroll && container) {
      container.scrollTop = container.scrollHeight;
    }
  });

  function handleScroll() {
    if (container) {
      const isAtBottom = container.scrollHeight - container.scrollTop - container.clientHeight < 50;
      autoScroll = isAtBottom;
    }
  }

  function formatTime(date: Date): string {
    const lang = $locale || undefined;
    return date.toLocaleTimeString(lang, { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  }
</script>

<div
  bind:this={container}
  class="h-full overflow-y-auto font-mono text-sm p-4"
  data-testid="log-viewer"
  onscroll={handleScroll}
>
  {#if logs.length === 0}
    <div class="text-vscode-muted text-center py-8">
      {$_('log.waiting')}
    </div>
  {:else}
    {#each logs as log, i (i)}
      <div class="flex gap-2 hover:bg-vscode-hover py-0.5 {log.isStderr ? 'text-vscode-error' : 'text-vscode'}" data-testid="log-line">
        <span class="text-vscode-muted shrink-0">[#{log.iteration}]</span>
        <span class="text-vscode-muted shrink-0">{formatTime(log.timestamp)}</span>
        <span class="break-all">{log.content}</span>
      </div>
    {/each}
  {/if}

  {#if !autoScroll && logs.length > 0}
    <button
      class="fixed bottom-20 right-8 px-3 py-1 bg-vscode-accent bg-vscode-accent-hover text-white rounded-full text-xs shadow-lg"
      onclick={() => {
        autoScroll = true;
        container.scrollTop = container.scrollHeight;
      }}
    >
      ↓ {$_('log.scrollBottom')}
    </button>
  {/if}
</div>
