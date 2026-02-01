<script lang="ts">
  import type { LogEntry } from "$lib/types";
  import { _, locale } from "svelte-i18n";

  interface Props {
    logs: LogEntry[];
    showHeader?: boolean;
  }

  interface ExtendedLogEntry extends LogEntry {
    isThinking?: boolean;
    lines?: LogEntry[];
  }

  let { logs, showHeader = false }: Props = $props();
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
      const isAtBottom =
        container.scrollHeight - container.scrollTop - container.clientHeight <
        50;
      autoScroll = isAtBottom;
    }
  }

  function formatTime(date: Date): string {
    const lang = $locale || undefined;
    return date.toLocaleTimeString(lang, {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  }
</script>

<div
  bind:this={container}
  class="h-full overflow-y-auto font-mono text-sm p-4"
  data-testid="log-viewer"
  onscroll={handleScroll}
>
  {#if showHeader}
    <div class="sticky top-0 z-10 pb-3">
      <slot name="header"></slot>
    </div>
  {/if}

  {#if logs.length === 0}
    <div class="text-vscode-muted text-center py-8">
      {$_("log.waiting")}
    </div>
  {:else}
    <!-- Log rendering with thinking block support -->
    {@const groupedLogs = (() => {
      const result: ExtendedLogEntry[] = [];
      let thinkingBlock: ExtendedLogEntry | null = null;

      for (const log of logs) {
        let content = log.content;

        // Case 1: Start of thinking block
        if (content.includes("<thinking>")) {
          // If we were already in a block, close it and push (shouldn't happen ideally but for safety)
          if (thinkingBlock) {
            result.push(thinkingBlock);
          }

          // Check if it closes on the same line
          if (content.includes("</thinking>")) {
            // It's a single line thinking block
            const parts = content.split(/<\/?thinking>/);
            // parts might be ["prefix", "thinking content", "suffix"] or similar
            // For simplicity, we just strip the tags and show it as a thinking block
            result.push({
              ...log,
              content: content.replace(/<\/?thinking>/g, ""),
              isThinking: true,
              lines: [],
            });
            thinkingBlock = null;
          } else {
            // Multi-line start
            thinkingBlock = {
              ...log,
              content: content.replace("<thinking>", ""),
              isThinking: true,
              lines: [],
            };
          }
        }
        // Case 2: End of thinking block
        else if (thinkingBlock && content.includes("</thinking>")) {
          if (thinkingBlock.lines) {
            thinkingBlock.lines.push({
              ...log,
              content: content.replace("</thinking>", ""),
            });
          }
          result.push(thinkingBlock);
          thinkingBlock = null;
        }
        // Case 3: Inside thinking block
        else if (thinkingBlock) {
          if (thinkingBlock.lines) {
            thinkingBlock.lines.push(log);
          }
        }
        // Case 4: Normal log
        else {
          result.push(log);
        }
      }
      if (thinkingBlock) result.push(thinkingBlock);
      return result;
    })()}

    {#each groupedLogs as log, i (i)}
      {#if log.isThinking}
        <div class="border-l-2 border-vscode-accent ml-2 pl-2 my-1">
          <details open>
            <summary
              class="cursor-pointer text-vscode-accent text-xs font-bold select-none hover:text-vscode-accent-hover flex items-center gap-2"
            >
              <span>{$_("log.thinkingProcess")}</span>
            </summary>
            <div class="mt-1 opacity-80">
              <div class="flex gap-2 py-0.5 text-xs text-vscode-muted">
                <span class="shrink-0">[#{log.iteration}]</span>
                <span class="shrink-0">{formatTime(log.timestamp)}</span>
                <span class="break-words whitespace-pre-wrap"
                  >{log.content}</span
                >
              </div>
              {#each log.lines as innerLog}
                <div class="flex gap-2 py-0.5 text-xs text-vscode-muted">
                  <span class="shrink-0">[#{innerLog.iteration}]</span>
                  <span class="shrink-0">{formatTime(innerLog.timestamp)}</span>
                  <span class="break-words whitespace-pre-wrap"
                    >{innerLog.content}</span
                  >
                </div>
              {/each}
            </div>
          </details>
        </div>
      {:else}
        <div
          class="flex gap-2 hover:bg-vscode-hover py-0.5 {log.isStderr
            ? 'text-vscode-error'
            : 'text-vscode'}"
          data-testid="log-line"
        >
          <span class="text-vscode-muted shrink-0 select-none"
            >[#{log.iteration}]</span
          >
          <span class="text-vscode-muted shrink-0 select-none"
            >{formatTime(log.timestamp)}</span
          >
          <span class="break-all whitespace-pre-wrap">{log.content}</span>
        </div>
      {/if}
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
      ↓ {$_("log.scrollBottom")}
    </button>
  {/if}
</div>
