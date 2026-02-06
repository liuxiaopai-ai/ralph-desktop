<script lang="ts">
  import type { ProjectState } from "$lib/types";
  import type { LoopStoreState } from "$lib/stores/loop";
  import * as api from "$lib/services/tauri";
  import { startLoopWithGuard } from "$lib/services/loopStart";
  import { updateCurrentProject } from "$lib/stores/projects";
  import { _ } from "svelte-i18n";
  import LogViewer from "./LogViewer.svelte";
  import PromptEditor from "./PromptEditor.svelte";

  interface Props {
    project: ProjectState;
    loopState: LoopStoreState;
  }

  let { project, loopState }: Props = $props();

  let starting = $state(false);
  let showPrompt = $state(false);
  let autoCommit = $state(true);
  let autoInitGit = $state(true);
  let isGitRepo = $state(false);
  let lastGitCheckId = $state<string | null>(null);
  const cliLabels: Record<string, string> = {
    claude: "Claude Code",
    codex: "Codex",
    opencode: "OpenCode",
  };

  const statusConfig = $derived({
    ready: {
      icon: "⚪",
      color: "text-vscode-muted",
      label: $_("task.status.ready"),
    },
    queued: {
      icon: "🔵",
      color: "text-vscode-info",
      label: $_("task.status.queued"),
    },
    running: {
      icon: "🟢",
      color: "text-vscode-success",
      label: $_("task.status.running"),
    },
    pausing: {
      icon: "🟡",
      color: "text-vscode-warning",
      label: $_("task.status.pausing"),
    },
    paused: {
      icon: "🟡",
      color: "text-vscode-warning",
      label: $_("task.status.paused"),
    },
    done: {
      icon: "✅",
      color: "text-vscode-success",
      label: $_("task.status.done"),
    },
    partial: {
      icon: "🔵",
      color: "text-vscode-info",
      label: $_("task.status.partial"),
    },
    failed: {
      icon: "❌",
      color: "text-vscode-error",
      label: $_("task.status.failed"),
    },
    cancelled: {
      icon: "🚫",
      color: "text-vscode-muted",
      label: $_("task.status.cancelled"),
    },
    brainstorming: {
      icon: "💭",
      color: "text-vscode-accent",
      label: $_("task.status.brainstorming"),
    },
  });

  const status = $derived(statusConfig[project.status] || statusConfig.ready);
  const isRunning = $derived(project.status === "running");
  const isPaused = $derived(project.status === "paused");
  const isPausing = $derived(project.status === "pausing");
  const isDone = $derived(project.status === "done");
  const isPartial = $derived(project.status === "partial");
  const isFailed = $derived(project.status === "failed");
  const showStatusBanner = $derived(isDone || isFailed || isPartial);
  const showStatusCard = $derived(isDone || isFailed || isPartial);
  const canStart = $derived(
    ["ready", "failed", "cancelled", "partial"].includes(project.status),
  );
  const showGitInit = $derived(canStart && !isGitRepo && !!project.task);
  const autoCommitEnabled = $derived(isGitRepo || autoInitGit);
  const showAutoCommit = $derived(canStart && !!project.task);
  const summaryText = $derived(loopState.summary || $_("task.summaryFallback"));
  const elapsedText = $derived(formatDuration(loopState.elapsedMs));
  const maxIterations = $derived(
    project.task?.maxIterations || loopState.maxIterations || 0,
  );

  const badgeConfig = $derived({
    done: "bg-vscode-success text-white border-vscode-success shadow-md shadow-black/20 animate-pulse",
    partial:
      "bg-vscode-info text-white border-vscode-info shadow-md shadow-black/20",
    failed:
      "bg-vscode-error text-white border-vscode-error shadow-md shadow-black/20 animate-pulse",
    running: "bg-vscode-panel text-vscode-success border-vscode-success",
    pausing: "bg-vscode-panel text-vscode-warning border-vscode-warning",
    paused: "bg-vscode-panel text-vscode-warning border-vscode-warning",
    queued: "bg-vscode-panel text-vscode-info border-vscode",
    ready: "bg-vscode-panel text-vscode-muted border-vscode",
    cancelled: "bg-vscode-panel text-vscode-muted border-vscode",
    brainstorming: "bg-vscode-panel text-vscode-accent border-vscode",
  });

  const bannerClass = $derived(
    isDone
      ? "bg-vscode-success"
      : isPartial
        ? "bg-vscode-info"
        : "bg-vscode-error",
  );

  $effect(() => {
    if (!project?.id) return;
    autoCommit = project.task?.autoCommit ?? true;
    autoInitGit = project.task?.autoInitGit ?? true;
    if (lastGitCheckId !== project.id) {
      lastGitCheckId = project.id;
      void refreshGitRepo(project.id);
    }
  });

  async function refreshGitRepo(projectId: string) {
    try {
      isGitRepo = await api.checkProjectGitRepo(projectId);
    } catch (error) {
      console.error("Failed to check git repo:", error);
      isGitRepo = false;
    }
  }

  async function handleAutoInitChange() {
    if (!project?.id) return;
    const next = autoInitGit;
    try {
      const updated = await api.updateTaskAutoInit(project.id, next);
      updateCurrentProject(updated);
    } catch (error) {
      console.error("Failed to update auto init git:", error);
      autoInitGit = !next;
    }
  }

  async function handleAutoCommitChange() {
    if (!project?.id || !autoCommitEnabled) return;
    const next = autoCommit;
    try {
      const updated = await api.updateTaskAutoCommit(project.id, next);
      updateCurrentProject(updated);
    } catch (error) {
      console.error("Failed to update auto commit:", error);
      autoCommit = !next;
    }
  }

  async function handleStart() {
    starting = true;
    try {
      if (showGitInit && autoInitGit) {
        await api.initProjectGitRepo(project.id);
        const updated = await api.setProjectSkipGitRepoCheck(project.id, false);
        updateCurrentProject(updated);
        isGitRepo = true;
      } else if (showGitInit && !autoInitGit && project.task?.cli === "codex") {
        const updated = await api.setProjectSkipGitRepoCheck(project.id, true);
        updateCurrentProject(updated);
      }
      await startLoopWithGuard(project.id);
    } catch (error) {
      console.error("Failed to start loop:", error);
    } finally {
      starting = false;
    }
  }

  async function handlePause() {
    try {
      await api.pauseLoop(project.id);
    } catch (error) {
      console.error("Failed to pause loop:", error);
    }
  }

  async function handleResume() {
    try {
      await api.resumeLoop(project.id);
    } catch (error) {
      console.error("Failed to resume loop:", error);
    }
  }

  async function handleStop() {
    if (confirm($_("task.stopConfirm"))) {
      try {
        await api.stopLoop(project.id);
      } catch (error) {
        console.error("Failed to stop loop:", error);
      }
    }
  }

  function formatDuration(ms: number | null): string {
    if (ms === null || Number.isNaN(ms)) return $_("task.durationUnknown");
    const totalSeconds = Math.max(0, Math.floor(ms / 1000));
    const hours = Math.floor(totalSeconds / 3600);
    const minutes = Math.floor((totalSeconds % 3600) / 60);
    const seconds = totalSeconds % 60;
    if (hours > 0) {
      return `${hours}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
    }
    return `${minutes}:${String(seconds).padStart(2, "0")}`;
  }

  function scrollToLogs() {
    const el = document.querySelector('[data-testid="log-viewer"]');
    if (el) {
      el.scrollIntoView({ behavior: "smooth", block: "start" });
    }
  }

  async function handleOpenProject() {
    const opener = await import("@tauri-apps/plugin-opener");
    await opener.openPath(project.path);
  }

  async function handleCopySummary() {
    await navigator.clipboard.writeText(summaryText);
  }

  async function handleCopyError() {
    const message = loopState.lastError || $_("task.errorFallback");
    await navigator.clipboard.writeText(message);
  }

  async function handleIncreaseIterations() {
    const current = project.task?.maxIterations || loopState.maxIterations || 0;
    const suggested = Math.max(current + 5, 1);
    const input = prompt(
      $_("task.actions.increaseIterationsPrompt", {
        values: { current, suggested },
      }),
      String(suggested),
    );
    if (!input) return;
    const next = Number.parseInt(input, 10);
    if (!Number.isFinite(next) || next < 1) return;
    try {
      const updated = await api.updateTaskMaxIterations(project.id, next);
      updateCurrentProject(updated);
      await handleStart();
    } catch (error) {
      console.error("Failed to update max iterations:", error);
    }
  }

  async function handlePromptSave(prompt: string) {
    try {
      const updated = await api.updateTaskPrompt(project.id, prompt);
      updateCurrentProject(updated);
    } catch (error) {
      console.error("Failed to update prompt:", error);
    }
  }
</script>

<div class="flex-1 flex flex-col overflow-hidden">
  {#if showStatusBanner}
    <div
      class="px-4 py-3 border-b border-vscode {bannerClass} text-white"
      data-testid="task-status-banner"
    >
      <div class="flex items-start justify-between gap-4">
        <div class="flex items-start gap-3 min-w-0">
          <div
            class="w-9 h-9 rounded-full bg-white/20 flex items-center justify-center text-xl flex-shrink-0"
          >
            {#if isDone}✓{:else if isPartial}✓{:else}✕{/if}
          </div>
          <div class="min-w-0">
            <div class="text-sm font-semibold">
              {isDone
                ? $_("task.banner.completedTitle")
                : isPartial
                  ? $_("task.banner.partialTitle")
                  : $_("task.banner.failedTitle")}
            </div>
            <div class="text-xs opacity-90">
              {$_("task.banner.meta", {
                values: {
                  duration: elapsedText,
                  current: loopState.currentIteration,
                  max: maxIterations,
                },
              })}
            </div>
            {#if isDone}
              <div class="text-xs opacity-90 mt-1 whitespace-pre-wrap">
                {summaryText}
              </div>
            {:else if isPartial}
              <div class="text-xs opacity-90 mt-1">
                {$_("task.banner.partialMessage")}
              </div>
            {/if}
          </div>
        </div>
        <div class="flex items-center gap-2 flex-shrink-0">
          <button
            class="px-3 py-1.5 text-xs rounded bg-white/15 hover:bg-white/25 transition"
            onclick={scrollToLogs}
          >
            {$_("task.actions.viewLogs")}
          </button>
          {#if isDone}
            <button
              class="px-3 py-1.5 text-xs rounded bg-white/15 hover:bg-white/25 transition"
              onclick={handleOpenProject}
            >
              {$_("task.actions.openProject")}
            </button>
            <button
              class="px-3 py-1.5 text-xs rounded bg-white/15 hover:bg-white/25 transition"
              onclick={handleCopySummary}
            >
              {$_("task.actions.copySummary")}
            </button>
          {:else if isPartial}
            <button
              class="px-3 py-1.5 text-xs rounded bg-white/15 hover:bg-white/25 transition"
              onclick={handleIncreaseIterations}
            >
              {$_("task.actions.increaseIterations")}
            </button>
            <button
              class="px-3 py-1.5 text-xs rounded bg-white/15 hover:bg-white/25 transition"
              onclick={handleOpenProject}
            >
              {$_("task.actions.useCurrent")}
            </button>
          {:else}
            <button
              class="px-3 py-1.5 text-xs rounded bg-white/15 hover:bg-white/25 transition"
              onclick={handleCopyError}
            >
              {$_("task.actions.copyError")}
            </button>
            <button
              class="px-3 py-1.5 text-xs rounded bg-white/15 hover:bg-white/25 transition"
              onclick={handleStart}
            >
              {$_("task.actions.retry")}
            </button>
          {/if}
        </div>
      </div>
    </div>
  {/if}

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
        <div
          class="px-3 py-1.5 rounded-full border text-sm font-semibold flex items-center gap-2 {badgeConfig[
            project.status
          ] || badgeConfig.ready}"
          data-testid="task-status"
          data-status={project.status}
        >
          <span class="text-base">{status.icon}</span>
          <span>{status.label}</span>
        </div>
      </div>
    </div>

    <!-- Task Info -->
    {#if project.task}
      <div class="mt-4 p-3 bg-vscode-input rounded-lg border border-vscode">
        <div class="flex items-center justify-between mb-2">
          <div class="grid grid-cols-3 gap-4 text-sm flex-1">
            <div>
              <span class="text-vscode-muted">{$_("task.cli")}:</span>
              <span class="ml-2 text-vscode font-medium">
                {cliLabels[project.task.cli] || project.task.cli}
              </span>
            </div>
            <div>
              <span class="text-vscode-muted">{$_("task.iteration")}:</span>
              <span class="ml-2 text-vscode font-medium">
                {loopState.currentIteration} / {project.task.maxIterations}
              </span>
            </div>
            <div>
              <span class="text-vscode-muted">{$_("task.statusLabel")}:</span>
              <span class="ml-2 {status.color} font-medium">{status.label}</span
              >
            </div>
          </div>
          <button
            class="ml-4 px-3 py-1 text-sm bg-vscode-panel border border-vscode hover:bg-vscode-hover rounded text-vscode-dim"
            onclick={() => (showPrompt = !showPrompt)}
          >
            {showPrompt ? $_("task.hidePrompt") : $_("task.showPrompt")}
          </button>
        </div>
        {#if showPrompt}
          <div class="mt-3">
            <PromptEditor {project} onSave={handlePromptSave} />
          </div>
        {/if}
      </div>
    {/if}
  </div>

  <!-- Log Viewer -->
  <div class="flex-1 overflow-hidden bg-vscode-editor">
    <LogViewer logs={loopState.logs} showHeader={showStatusCard}>
      <svelte:fragment slot="header">
        {#if showStatusCard}
          <div
            class="bg-vscode-panel border border-vscode rounded-lg p-4 shadow-md"
          >
            <div class="flex items-center justify-between gap-3">
              <div class="flex items-center gap-2">
                <span class="text-lg"
                  >{isDone ? "✅" : isPartial ? "🔵" : "❌"}</span
                >
                <div class="text-sm font-semibold text-vscode">
                  {isDone
                    ? $_("task.statusCard.completedTitle")
                    : isPartial
                      ? $_("task.statusCard.partialTitle")
                      : $_("task.statusCard.failedTitle")}
                </div>
              </div>
              <div class="text-xs text-vscode-muted">
                {$_("task.banner.meta", {
                  values: {
                    duration: elapsedText,
                    current: loopState.currentIteration,
                    max: maxIterations,
                  },
                })}
              </div>
            </div>
            {#if isDone}
              <div class="text-sm text-vscode mt-3">{summaryText}</div>
            {:else if isPartial}
              <div class="text-sm text-vscode mt-3">
                {$_("task.statusCard.partialMessage")}
              </div>
            {:else}
              <div class="text-sm text-vscode-error mt-3">
                {loopState.lastError || $_("task.errorFallback")}
              </div>
            {/if}
          </div>
        {/if}
      </svelte:fragment>
    </LogViewer>
  </div>

  <!-- Control Bar -->
  <div class="p-4 bg-vscode-panel border-t border-vscode">
    <div class="flex items-center justify-between">
      <div class="flex flex-col gap-3">
        {#if showGitInit}
          <div
            class="rounded-lg border border-vscode bg-vscode-panel px-3 py-2 text-xs text-vscode"
          >
            <div class="flex items-start gap-2">
              <input
                id={`auto-init-git-${project.id}`}
                type="checkbox"
                class="mt-0.5"
                bind:checked={autoInitGit}
                onchange={handleAutoInitChange}
              />
              <div class="min-w-0">
                <label
                  for={`auto-init-git-${project.id}`}
                  class="text-vscode font-medium"
                >
                  {$_("task.autoInitGit.label")}
                </label>
                <div class="text-vscode-muted mt-1">
                  {$_("task.autoInitGit.description")}
                </div>
              </div>
            </div>
          </div>
        {/if}

        {#if showAutoCommit}
          <div
            class={`mb-3 rounded-lg border border-vscode bg-vscode-panel px-3 py-2 text-xs text-vscode ${!autoCommitEnabled ? "opacity-50" : ""}`}
          >
            <div class="flex items-start gap-2">
              <input
                id={`auto-commit-${project.id}`}
                type="checkbox"
                class="mt-0.5"
                bind:checked={autoCommit}
                disabled={!autoCommitEnabled}
                onchange={handleAutoCommitChange}
              />
              <div class="min-w-0">
                <label
                  for={`auto-commit-${project.id}`}
                  class="text-vscode font-medium"
                >
                  {$_("task.autoCommit.label")}
                </label>
                <div class="text-vscode-muted mt-1">
                  {$_("task.autoCommit.description")}
                </div>
                <div class="text-vscode-muted mt-1">
                  {$_("task.autoCommit.note")}
                </div>
                {#if !autoCommitEnabled}
                  <div class="text-vscode-muted mt-1">
                    {$_("task.autoCommit.requiresGit")}
                  </div>
                {/if}
              </div>
            </div>
          </div>
        {/if}

        <div class="flex gap-2">
          {#if canStart}
            <button
              class="px-4 py-2 bg-vscode-accent bg-vscode-accent-hover text-white rounded-lg flex items-center gap-2 disabled:opacity-50"
              onclick={handleStart}
              disabled={starting}
              data-testid="task-start"
            >
              <span>▶</span>
              <span>{starting ? $_("task.starting") : $_("task.start")}</span>
            </button>
          {/if}

          {#if isRunning}
            <button
              class="px-4 py-2 bg-vscode-warning text-black rounded-lg flex items-center gap-2 hover:opacity-90"
              onclick={handlePause}
              data-testid="task-pause"
            >
              <span>⏸</span>
              <span>{$_("task.pause")}</span>
            </button>
          {/if}

          {#if isPaused}
            <button
              class="px-4 py-2 bg-vscode-accent bg-vscode-accent-hover text-white rounded-lg flex items-center gap-2"
              onclick={handleResume}
              data-testid="task-resume"
            >
              <span>▶</span>
              <span>{$_("task.resume")}</span>
            </button>
          {/if}

          {#if isRunning || isPaused || isPausing}
            <button
              class="px-4 py-2 bg-vscode-error text-white rounded-lg flex items-center gap-2 hover:opacity-90"
              onclick={handleStop}
              data-testid="task-stop"
            >
              <span>⏹</span>
              <span>{$_("task.stop")}</span>
            </button>
          {/if}
        </div>
      </div>

      {#if loopState.lastError}
        <div class="text-sm text-vscode-error">
          {$_("task.errorPrefix")}
          {loopState.lastError}
        </div>
      {/if}
    </div>
  </div>
</div>
