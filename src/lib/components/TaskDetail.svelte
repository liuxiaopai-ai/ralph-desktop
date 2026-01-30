<script lang="ts">
  import type { ProjectState, LogEntry } from '$lib/types';
  import type { LoopStoreState } from '$lib/stores/loop';
  import * as api from '$lib/services/tauri';
  import { startLoopWithGuard } from '$lib/services/loopStart';
  import { _ } from 'svelte-i18n';
  import LogViewer from './LogViewer.svelte';
  import PromptEditor from './PromptEditor.svelte';

  interface Props {
    project: ProjectState;
    loopState: LoopStoreState;
  }

  let { project, loopState }: Props = $props();

  let starting = $state(false);
  let showPrompt = $state(false);
  const cliLabels: Record<string, string> = {
    claude: 'Claude Code',
    codex: 'Codex',
    opencode: 'OpenCode'
  };

  const statusConfig = $derived({
    ready: { icon: '⚪', color: 'text-vscode-muted', label: $_('task.status.ready') },
    queued: { icon: '🔵', color: 'text-vscode-info', label: $_('task.status.queued') },
    running: { icon: '🟢', color: 'text-vscode-success', label: $_('task.status.running') },
    pausing: { icon: '🟡', color: 'text-vscode-warning', label: $_('task.status.pausing') },
    paused: { icon: '🟡', color: 'text-vscode-warning', label: $_('task.status.paused') },
    done: { icon: '✅', color: 'text-vscode-success', label: $_('task.status.done') },
    failed: { icon: '❌', color: 'text-vscode-error', label: $_('task.status.failed') },
    cancelled: { icon: '🚫', color: 'text-vscode-muted', label: $_('task.status.cancelled') },
    brainstorming: { icon: '💭', color: 'text-vscode-accent', label: $_('task.status.brainstorming') }
  });

  const status = $derived(statusConfig[project.status] || statusConfig.ready);
  const isRunning = $derived(project.status === 'running');
  const isPaused = $derived(project.status === 'paused');
  const isPausing = $derived(project.status === 'pausing');
  const canStart = $derived(['ready', 'failed', 'cancelled'].includes(project.status));

  async function handleStart() {
    starting = true;
    try {
      await startLoopWithGuard(project.id);
    } catch (error) {
      console.error('Failed to start loop:', error);
    } finally {
      starting = false;
    }
  }

  async function handlePause() {
    try {
      await api.pauseLoop(project.id);
    } catch (error) {
      console.error('Failed to pause loop:', error);
    }
  }

  async function handleResume() {
    try {
      await api.resumeLoop(project.id);
    } catch (error) {
      console.error('Failed to resume loop:', error);
    }
  }

  async function handleStop() {
    if (confirm($_('task.stopConfirm'))) {
      try {
        await api.stopLoop(project.id);
      } catch (error) {
        console.error('Failed to stop loop:', error);
      }
    }
  }
</script>

<div class="flex-1 flex flex-col overflow-hidden">
  <!-- Header -->
  <div class="p-4 bg-vscode-panel border-b border-vscode">
    <div class="flex items-start justify-between">
      <div>
        <div class="flex items-center gap-2">
          <span class="text-2xl">📁</span>
          <h2 class="text-xl font-bold text-vscode">{project.name}</h2>
        </div>
        <p class="text-sm text-vscode-dim mt-1">{project.path}</p>
      </div>
      <div class="flex items-center gap-2">
        <span class={status.color}>{status.icon}</span>
        <span class="text-sm font-medium text-vscode-dim">{status.label}</span>
      </div>
    </div>

    <!-- Task Info -->
    {#if project.task}
      <div class="mt-4 p-3 bg-vscode-input rounded-lg border border-vscode">
        <div class="flex items-center justify-between mb-2">
          <div class="grid grid-cols-3 gap-4 text-sm flex-1">
            <div>
              <span class="text-vscode-muted">{$_('task.cli')}:</span>
              <span class="ml-2 text-vscode font-medium">
                {cliLabels[project.task.cli] || project.task.cli}
              </span>
            </div>
            <div>
              <span class="text-vscode-muted">{$_('task.iteration')}:</span>
              <span class="ml-2 text-vscode font-medium">
                {loopState.currentIteration} / {project.task.maxIterations}
              </span>
            </div>
            <div>
              <span class="text-vscode-muted">{$_('task.statusLabel')}:</span>
              <span class="ml-2 {status.color} font-medium">{status.label}</span>
            </div>
          </div>
          <button
            class="ml-4 px-3 py-1 text-sm bg-vscode-panel border border-vscode hover:bg-vscode-hover rounded text-vscode-dim"
            onclick={() => showPrompt = !showPrompt}
          >
            {showPrompt ? $_('task.hidePrompt') : $_('task.showPrompt')}
          </button>
        </div>
        {#if showPrompt}
          <div class="mt-3">
            <PromptEditor {project} />
          </div>
        {/if}
      </div>
    {/if}
  </div>

  <!-- Log Viewer -->
  <div class="flex-1 overflow-hidden bg-vscode-editor">
    <LogViewer logs={loopState.logs} />
  </div>

  <!-- Control Bar -->
  <div class="p-4 bg-vscode-panel border-t border-vscode">
    <div class="flex items-center justify-between">
      <div class="flex gap-2">
        {#if canStart}
          <button
            class="px-4 py-2 bg-vscode-accent bg-vscode-accent-hover text-white rounded-lg flex items-center gap-2 disabled:opacity-50"
            onclick={handleStart}
            disabled={starting}
          >
            <span>▶</span>
            <span>{starting ? $_('task.starting') : $_('task.start')}</span>
          </button>
        {/if}

        {#if isRunning}
          <button
            class="px-4 py-2 bg-vscode-warning text-black rounded-lg flex items-center gap-2 hover:opacity-90"
            onclick={handlePause}
          >
            <span>⏸</span>
            <span>{$_('task.pause')}</span>
          </button>
        {/if}

        {#if isPaused}
          <button
            class="px-4 py-2 bg-vscode-accent bg-vscode-accent-hover text-white rounded-lg flex items-center gap-2"
            onclick={handleResume}
          >
            <span>▶</span>
            <span>{$_('task.resume')}</span>
          </button>
        {/if}

        {#if isRunning || isPaused || isPausing}
          <button
            class="px-4 py-2 bg-vscode-error text-white rounded-lg flex items-center gap-2 hover:opacity-90"
            onclick={handleStop}
          >
            <span>⏹</span>
            <span>{$_('task.stop')}</span>
          </button>
        {/if}
      </div>

      {#if loopState.lastError}
        <div class="text-sm text-vscode-error">
          {$_('task.errorPrefix')} {loopState.lastError}
        </div>
      {/if}
    </div>
  </div>
</div>
