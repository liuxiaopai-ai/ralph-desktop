<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { _ } from "svelte-i18n";
  import "$lib/i18n";
  import {
    projects,
    currentProjectId,
    currentProject,
    updateProjects,
    updateCurrentProject,
    updateProjectStatus,
    selectProject,
  } from "$lib/stores/projects";
  import {
    config,
    availableClis,
    updateConfig,
    setAvailableClis,
  } from "$lib/stores/settings";
  import {
    loopStates,
    getLoopState,
    setStatus,
    setIteration,
    addLog,
    setError,
    markStarted,
    markEnded,
    setSummary,
  } from "$lib/stores/loop";
  import {
    gitRepoCheckRequest,
    clearGitRepoCheck,
    requestGitRepoCheck,
  } from "$lib/stores/gitRepoCheck";
  import { dequeueProject, isInQueue, markRunning } from "$lib/stores/queue";
  import {
    notifySuccess,
    notifyError,
    notifyWarning,
  } from "$lib/stores/notifications";
  import { initTheme } from "$lib/stores/theme";
  import * as api from "$lib/services/tauri";
  import {
    CODEX_GIT_REPO_CHECK_REQUIRED,
    startLoopWithGuard,
  } from "$lib/services/loopStart";
  import { setLocaleFromConfig } from "$lib/i18n";
  import {
    initAutoUpdate,
    checkForUpdatesIfIdle,
  } from "$lib/stores/autoUpdate";
  import type { LoopEvent } from "$lib/types";
  import type { RecoveryInfo } from "$lib/services/tauri";
  import RecoveryDialog from "$lib/components/RecoveryDialog.svelte";
  import NotificationToast from "$lib/components/NotificationToast.svelte";
  import GitRepoCheckDialog from "$lib/components/GitRepoCheckDialog.svelte";

  const loopState = $derived(getLoopState($loopStates, $currentProjectId));
  let { children } = $props();
  let initialized = $state(false);
  let showPermissionDialog = $state(false);
  let showRecoveryDialog = $state(false);
  let interruptedTasks = $state<RecoveryInfo[]>([]);
  let gitRepoBusy = $state(false);

  // Ensure i18n has an initial locale before first render.
  setLocaleFromConfig("system");

  onMount(async () => {
    try {
      // Load config
      const loadedConfig = await api.getConfig();
      updateConfig(loadedConfig);
      setLocaleFromConfig(loadedConfig.language);
      initTheme(loadedConfig.theme);

      // Check if permissions need confirmation
      if (!loadedConfig.permissionsConfirmed) {
        showPermissionDialog = true;
      }

      // Detect CLIs
      const clis = await api.detectInstalledClis();
      setAvailableClis(clis);

      // Load projects
      const projectList = await api.listProjects();
      updateProjects(projectList);

      // Check for interrupted tasks
      const interrupted = await api.checkInterruptedTasks();
      if (interrupted.length > 0) {
        interruptedTasks = interrupted;
        showRecoveryDialog = true;
      }

      // Listen to loop events
      await api.listenToLoopEvents(handleLoopEvent);

      // Start auto-update scheduler
      initAutoUpdate(isIdleForUpdate);
      checkForUpdatesIfIdle(isIdleForUpdate());

      // Clean up old logs on startup
      await api.cleanupLogs();

      initialized = true;
    } catch (error) {
      console.error("Initialization error:", error);
      initialized = true; // Still show UI even on error
    }
  });

  function isIdleForUpdate(): boolean {
    const running =
      loopState?.status === "running" || loopState?.status === "pausing";
    const queued = $projects?.some((p) => p.status === "queued");
    return !running && !queued;
  }

  function handleLoopEvent(event: LoopEvent) {
    const statusMap: Record<string, string> = {
      iterationStart: "running",
      pausing: "pausing",
      paused: "paused",
      resumed: "running",
      completed: "done",
      maxIterationsReached: "partial",
      error: "failed",
      stopped: "cancelled",
    };

    const projectId = event.projectId;
    if (!projectId) {
      return;
    }

    if (event.type === "output" && event.content) {
      addLog(projectId, {
        iteration: event.iteration || 0,
        content: event.content,
        isStderr: event.isStderr || false,
        timestamp: new Date(),
      });
    }

    if (event.type === "iterationStart" && event.iteration === 1) {
      markStarted(projectId, new Date());
    }

    if (event.type === "error" && event.error) {
      if (event.error.includes(CODEX_GIT_REPO_CHECK_REQUIRED)) {
        requestGitRepoCheck(event.projectId, "runtime");
        return;
      }
      setError(projectId, event.error);
      markEnded(projectId, new Date());
      notifyError($_("notifications.executionErrorTitle"), event.error);
    }

    if (event.type === "completed") {
      markEnded(projectId, new Date());
      const summary = buildSummary(projectId);
      if (summary) {
        setSummary(projectId, summary);
      }
      notifySuccess(
        $_("notifications.taskCompletedTitle"),
        $_("notifications.taskCompletedMessage"),
      );
    }

    if (event.type === "maxIterationsReached") {
      markEnded(projectId, new Date());
      const summary = buildSummary(projectId);
      if (summary) {
        setSummary(projectId, summary);
      }
      notifyWarning(
        $_("notifications.maxIterationsTitle"),
        $_("notifications.maxIterationsMessage", {
          values: { iteration: event.iteration },
        }),
      );
    }

    if (event.iteration !== undefined) {
      setIteration(projectId, event.iteration);
    }

    const newStatus = statusMap[event.type];
    if (newStatus) {
      setStatus(projectId, newStatus as any);
      updateProjectStatus(event.projectId, newStatus as any);
      checkForUpdatesIfIdle(isIdleForUpdate());
    }
  }

  function buildSummary(projectId: string): string | null {
    const state = getLoopState(get(loopStates), projectId);
    const project = get(currentProject);
    const completionSignal =
      project?.id === projectId
        ? (project?.task?.completionSignal ?? "<done>COMPLETE</done>")
        : "<done>COMPLETE</done>";
    const parts: string[] = [];
    let total = 0;
    const maxLen = 1000;

    for (let i = state.logs.length - 1; i >= 0; i--) {
      const entry = state.logs[i];
      if (entry.isStderr) continue;
      const text = entry.content?.trim();
      if (!text) continue;
      if (text.includes(completionSignal)) continue;
      if (text.startsWith("{") && text.endsWith("}")) continue;
      if (text.startsWith("[") && text.endsWith("]")) continue;
      parts.unshift(text);
      total += text.length;
      if (total >= maxLen) break;
    }

    const summary = parts.join(" ").replace(/\s+/g, " ").trim();
    if (!summary) {
      return $_("task.summaryFallback");
    }
    return summary.length > maxLen
      ? `${summary.slice(0, maxLen - 1)}…`
      : summary;
  }

  async function handleConfirmPermissions() {
    await api.confirmPermissions();
    const loadedConfig = await api.getConfig();
    updateConfig(loadedConfig);
    initTheme(loadedConfig.theme);
    showPermissionDialog = false;
  }

  function handleRecoverTask(projectId: string) {
    selectProject(projectId);
    interruptedTasks = interruptedTasks.filter(
      (t) => t.projectId !== projectId,
    );
    if (interruptedTasks.length === 0) {
      showRecoveryDialog = false;
    }
  }

  function handleCancelTask(projectId: string) {
    interruptedTasks = interruptedTasks.filter(
      (t) => t.projectId !== projectId,
    );
    if (interruptedTasks.length === 0) {
      showRecoveryDialog = false;
    }
  }

  function handleDismissRecovery() {
    showRecoveryDialog = false;
  }

  async function startLoopFromDialog(projectId: string) {
    const started = await startLoopWithGuard(projectId);
    if (started && isInQueue(projectId)) {
      dequeueProject(projectId);
      markRunning(projectId);
    }
  }

  async function handleInitGitRepo(projectId: string) {
    gitRepoBusy = true;
    try {
      await api.initProjectGitRepo(projectId);
      clearGitRepoCheck();
      await startLoopFromDialog(projectId);
    } catch (error) {
      console.error("Failed to init git repo:", error);
      notifyError($_("notifications.gitInitFailed"), String(error));
    } finally {
      gitRepoBusy = false;
    }
  }

  async function handleSkipGitRepoCheck(projectId: string) {
    gitRepoBusy = true;
    try {
      const updated = await api.setProjectSkipGitRepoCheck(projectId, true);
      if ($currentProject && $currentProject.id === updated.id) {
        updateCurrentProject(updated);
      }
      clearGitRepoCheck();
      await startLoopFromDialog(projectId);
    } catch (error) {
      console.error("Failed to skip git repo check:", error);
      notifyError($_("notifications.skipGitFailed"), String(error));
    } finally {
      gitRepoBusy = false;
    }
  }

  function handleCancelGitRepoCheck() {
    clearGitRepoCheck();
  }
</script>

{#if showPermissionDialog}
  <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
    <div
      class="bg-vscode-panel border border-vscode rounded-lg shadow-xl max-w-lg p-6 m-4"
    >
      <h2 class="text-xl font-bold text-vscode-warning mb-4">
        ⚠️ {$_("permissions.title")}
      </h2>
      <div class="text-vscode space-y-3 mb-6">
        <p>{$_("permissions.intro")}</p>
        <ul class="list-disc list-inside space-y-1 ml-2">
          <li>{$_("permissions.bullet1")}</li>
          <li>{$_("permissions.bullet2")}</li>
          <li>{$_("permissions.bullet3")}</li>
        </ul>
        <p class="font-medium">{$_("permissions.recommendationTitle")}</p>
        <ul class="list-disc list-inside space-y-1 ml-2">
          <li>{$_("permissions.recommendation1")}</li>
          <li>{$_("permissions.recommendation2")}</li>
          <li>{$_("permissions.recommendation3")}</li>
        </ul>
      </div>
      <div class="flex justify-end gap-3">
        <button
          class="px-4 py-2 rounded-lg bg-vscode-panel border border-vscode text-vscode-dim hover:bg-vscode-hover"
          onclick={() => window.close()}
        >
          {$_("permissions.cancel")}
        </button>
        <button
          class="px-4 py-2 rounded-lg bg-vscode-accent bg-vscode-accent-hover text-white"
          onclick={handleConfirmPermissions}
        >
          {$_("permissions.confirm")}
        </button>
      </div>
    </div>
  </div>
{/if}

{#if showRecoveryDialog && interruptedTasks.length > 0}
  <RecoveryDialog
    tasks={interruptedTasks}
    onRecover={handleRecoverTask}
    onCancel={handleCancelTask}
    onDismiss={handleDismissRecovery}
  />
{/if}

{#if $gitRepoCheckRequest}
  {@const pending = $gitRepoCheckRequest}
  {@const meta = $projects.find((p) => p.id === pending.projectId)}
  <GitRepoCheckDialog
    projectName={meta?.name ||
      $currentProject?.name ||
      $_("app.unknownProject")}
    projectPath={meta?.path || $currentProject?.path || ""}
    reason={pending.reason}
    busy={gitRepoBusy}
    onInit={() => handleInitGitRepo(pending.projectId)}
    onSkip={() => handleSkipGitRepoCheck(pending.projectId)}
    onCancel={handleCancelGitRepoCheck}
  />
{/if}

{#if initialized}
  {@render children()}
{:else}
  <div class="flex items-center justify-center h-screen bg-vscode-editor">
    <div class="text-center">
      <div
        class="animate-spin rounded-full h-12 w-12 border-b-2 border-vscode-accent mx-auto mb-4"
      ></div>
      <p class="text-vscode-muted">{$_("common.loading")}</p>
    </div>
  </div>
{/if}

<NotificationToast />
