<script lang="ts">
  import type { LogEntry } from "$lib/types";
  import { _, locale } from "svelte-i18n";
  import { marked } from "marked";

  const allowedProtocols = new Set(["http:", "https:", "mailto:"]);

  function escapeHtml(str: string): string {
    return str
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll('"', "&quot;")
      .replaceAll("'", "&#39;");
  }

  function isSafeUrl(href: string | null): href is string {
    if (!href) return false;
    try {
      const url = new URL(href, "http://localhost");
      return allowedProtocols.has(url.protocol);
    } catch {
      return false;
    }
  }

  const renderer = new marked.Renderer();
  renderer.html = (html) => escapeHtml(html);
  renderer.text = (text) => escapeHtml(text);
  renderer.link = (href, title, text) => {
    if (!isSafeUrl(href)) return escapeHtml(text);
    const safeHref = escapeHtml(href);
    const safeTitle = title ? ` title="${escapeHtml(title)}"` : "";
    return `<a href="${safeHref}"${safeTitle} target="_blank" rel="noreferrer noopener">${text}</a>`;
  };
  renderer.image = (href, title, text) => {
    if (!isSafeUrl(href)) return escapeHtml(text);
    const safeSrc = escapeHtml(href);
    const safeTitle = title ? ` title="${escapeHtml(title)}"` : "";
    const safeAlt = escapeHtml(text);
    return `<img src="${safeSrc}" alt="${safeAlt}"${safeTitle} loading="lazy" />`;
  };

  // Configure marked to treat newlines as line breaks while enforcing safe output
  marked.use({ breaks: true, gfm: true, renderer });

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

  function enhanceMarkdown(content: string): string {
    // 1. Highlight commands: Lines starting with "> " or "$ "
    // Use 'cli' language to trigger custom CSS for seamless terminal styling
    let formatted = content.replace(/^([>$] .+$)/gm, "```cli\n$1\n```");

    // 2. Highlight edits: Lines starting with specific keywords
    formatted = formatted.replace(
      /^((?:Editing|Patching|Creating|Deleting|Reading) file:? .+$)/gm,
      "🔹 **$1**",
    );

    return formatted;
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
      let outputBlock: ExtendedLogEntry | null = null;

      function flushOutput() {
        if (outputBlock) {
          result.push(outputBlock);
          outputBlock = null;
        }
      }

      function flushThinking() {
        if (thinkingBlock) {
          result.push(thinkingBlock);
          thinkingBlock = null;
        }
      }

      for (const log of logs) {
        let content = log.content;

        // Case 1: Start of thinking block
        if (content.includes("<thinking>")) {
          flushOutput();

          if (thinkingBlock) flushThinking(); // Should not happen but safety

          if (content.includes("</thinking>")) {
            result.push({
              ...log,
              content: content.replace(/<\/?thinking>/g, ""),
              isThinking: true,
              lines: [],
            });
          } else {
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
          flushThinking();
        }
        // Case 3: Inside thinking block
        else if (thinkingBlock) {
          if (thinkingBlock.lines) {
            thinkingBlock.lines.push(log);
          }
        }
        // Case 4: Normal log (Output)
        else {
          // If we have an existing output block, check if we can merge
          // Merge if same type AND within time threshold (2s)
          if (
            outputBlock &&
            outputBlock.isStderr === log.isStderr &&
            new Date(log.timestamp).getTime() -
              new Date(outputBlock.timestamp).getTime() <
              2000
          ) {
            outputBlock.content += log.content;
          } else {
            flushOutput();
            outputBlock = { ...log };
          }
        }
      }
      flushOutput();
      flushThinking();

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
                <div
                  class="markdown-content prose prose-invert prose-sm max-w-none break-words"
                >
                  {@html marked.parse(enhanceMarkdown(log.content))}
                </div>
              </div>
              {#each log.lines as innerLog}
                <div class="flex gap-2 py-0.5 text-xs text-vscode-muted">
                  <span class="shrink-0">[#{innerLog.iteration}]</span>
                  <span class="shrink-0">{formatTime(innerLog.timestamp)}</span>
                  <div
                    class="markdown-content prose prose-invert prose-sm max-w-none break-words flex-1"
                  >
                    {@html marked.parse(enhanceMarkdown(innerLog.content))}
                  </div>
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
          <div
            class="markdown-content prose prose-invert prose-sm max-w-none break-words flex-1"
          >
            {@html marked.parse(enhanceMarkdown(log.content))}
          </div>
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
