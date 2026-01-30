<script lang="ts">
  import type { CliInfo } from '$lib/types';
  import { _ } from 'svelte-i18n';

  interface Props {
    clis: CliInfo[];
  }

  let { clis }: Props = $props();

  const installGuides = $derived({
    claude: {
      name: 'Claude Code',
      description: $_('cliMissing.claudeDesc'),
      installCmd: 'npm install -g @anthropic-ai/claude-code',
      docUrl: 'https://docs.anthropic.com/en/docs/claude-code',
      icon: '🤖'
    },
    codex: {
      name: 'Codex CLI',
      description: $_('cliMissing.codexDesc'),
      installCmd: 'npm install -g @openai/codex',
      docUrl: 'https://github.com/openai/codex-cli',
      icon: '⚡'
    },
    opencode: {
      name: 'OpenCode',
      description: $_('cliMissing.opencodeDesc'),
      installCmd: 'npm install -g opencode-ai',
      docUrl: 'https://opencode.ai/docs/cli/',
      icon: '🧩'
    }
  });

  async function openUrl(url: string) {
    const { open } = await import('@tauri-apps/plugin-opener');
    await open(url);
  }

  function copyToClipboard(text: string) {
    navigator.clipboard.writeText(text);
  }
</script>

<div class="flex-1 flex items-center justify-center p-8">
  <div class="max-w-lg w-full">
    <!-- Warning Icon -->
    <div class="text-center mb-6">
      <div class="inline-flex items-center justify-center w-16 h-16 rounded-full bg-[#f14c4c20] mb-4">
        <svg class="w-8 h-8 text-[#f14c4c]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"/>
        </svg>
      </div>
      <h2 class="text-xl font-semibold text-vscode mb-2">{$_('cliMissing.title')}</h2>
      <p class="text-sm text-vscode-dim">
        {$_('cliMissing.bodyLine1')}<br>
        {$_('cliMissing.bodyLine2')}
      </p>
    </div>

    <!-- Install Guides -->
    <div class="space-y-3">
      {#each Object.entries(installGuides) as [key, guide]}
        {@const cliInfo = clis.find(c => c.cliType === key)}
        <div class="bg-vscode-sidebar rounded-lg border border-vscode p-4">
          <div class="flex items-start gap-3">
            <span class="text-2xl">{guide.icon}</span>
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-2 mb-1">
                <h3 class="font-medium text-vscode">{guide.name}</h3>
                {#if cliInfo?.available}
                  <span class="px-1.5 py-0.5 text-xs rounded bg-[#4ec9b020] text-[#4ec9b0]">{$_('cliMissing.installed')}</span>
                {:else}
                  <span class="px-1.5 py-0.5 text-xs rounded bg-[#f14c4c20] text-[#f14c4c]">{$_('cliMissing.notInstalled')}</span>
                {/if}
              </div>
              <p class="text-xs text-vscode-muted mb-3">{guide.description}</p>

              <!-- Install Command -->
              <div class="flex items-center gap-2 mb-2">
                <code class="flex-1 px-2 py-1.5 bg-vscode-input rounded text-xs text-vscode font-mono truncate">
                  {guide.installCmd}
                </code>
                <button
                  class="px-2 py-1.5 text-xs bg-vscode-input hover:bg-vscode-hover rounded text-vscode-dim hover:text-vscode transition-colors"
                  onclick={() => copyToClipboard(guide.installCmd)}
                  title={$_('cliMissing.copyCommand')}
                >
                  <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"/>
                  </svg>
                </button>
              </div>

              <!-- Doc Link -->
              <button
                class="text-xs text-vscode-accent hover:underline"
                onclick={() => openUrl(guide.docUrl)}
              >
                {$_('cliMissing.viewDocs')}
              </button>
            </div>
          </div>
        </div>
      {/each}
    </div>

    <!-- Refresh Hint -->
    <div class="mt-6 text-center">
      <p class="text-xs text-vscode-muted mb-3">
        {$_('cliMissing.restartHint')}
      </p>
      <button
        class="px-4 py-2 bg-vscode-accent hover:bg-vscode-accent-hover text-white text-sm rounded transition-colors"
        onclick={() => window.location.reload()}
      >
        {$_('cliMissing.refresh')}
      </button>
    </div>
  </div>
</div>
