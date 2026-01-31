<script lang="ts">
  import { projects, currentProjectId, currentProject, selectProject, addProject, removeProject, updateCurrentProject } from '$lib/stores/projects';
  import { _ } from 'svelte-i18n';
  import { loopState, resetLoop, clearLogs } from '$lib/stores/loop';
  import { config, availableClis } from '$lib/stores/settings';
  import * as api from '$lib/services/tauri';
  import type { ProjectState, CliType } from '$lib/types';
  import ProjectList from '$lib/components/ProjectList.svelte';
  import TaskDetail from '$lib/components/TaskDetail.svelte';
  import AiBrainstorm from '$lib/components/AiBrainstorm.svelte';
  import SettingsPanel from '$lib/components/SettingsPanel.svelte';
  import QueueStatus from '$lib/components/QueueStatus.svelte';
  import ShortcutsHelp from '$lib/components/ShortcutsHelp.svelte';
  import KeyboardShortcuts from '$lib/components/KeyboardShortcuts.svelte';
  import CliNotInstalled from '$lib/components/CliNotInstalled.svelte';

  let showBrainstorm = $state(false);
  let showSettings = $state(false);
  let showShortcuts = $state(false);
  let creatingProject = $state(false);
  const isE2E = import.meta.env.VITE_E2E === '1';
  const e2eProjectPath = import.meta.env.VITE_E2E_PROJECT_PATH as string | undefined;

  // Keyboard shortcuts
  const shortcuts = $derived([
    { key: 'n', ctrl: true, action: handleCreateProject, description: $_('shortcuts.newProject') },
    { key: ',', ctrl: true, action: () => showSettings = true, description: $_('shortcuts.openSettings') },
    { key: '?', ctrl: true, action: () => showShortcuts = true, description: $_('shortcuts.showHelp') },
    { key: 'Escape', action: handleEscape, description: $_('shortcuts.closeDialog') },
  ]);

  function handleEscape() {
    if (showSettings) showSettings = false;
    else if (showShortcuts) showShortcuts = false;
    else if (showBrainstorm) showBrainstorm = false;
  }

  // Reactive: load project details when selection changes
  $effect(() => {
    const id = $currentProjectId;
    if (id) {
      loadProjectDetails(id);
    } else {
      updateCurrentProject(null as any);
    }
  });

  async function loadProjectDetails(id: string) {
    try {
      const project = await api.getProject(id);
      updateCurrentProject(project);

      // Check if need brainstorm
      if (project.status === 'brainstorming') {
        showBrainstorm = true;
      } else {
        showBrainstorm = false;
      }
    } catch (error) {
      console.error('Failed to load project:', error);
    }
  }

  async function handleCreateProject() {
    creatingProject = true;
    try {
      let selected: string | null = null;

      if (isE2E) {
        const ts = Date.now();
        selected = e2eProjectPath || `/tmp/ralph-e2e-${ts}`;
      } else {
        // Use Tauri dialog to select directory
        const { open } = await import('@tauri-apps/plugin-dialog');
        selected = await open({
          directory: true,
          multiple: false,
          title: $_('dialogs.selectProjectDir')
        }) as string | null;
      }

      if (selected) {
        const path = selected;
        const name = path.split('/').pop() || $_('app.newProject');
        const project = await api.createProject(path, name);
        addProject({
          id: project.id,
          name: project.name,
          path: project.path,
          status: project.status,
          createdAt: project.createdAt,
          lastOpenedAt: project.updatedAt
        });
        selectProject(project.id);
      }
    } catch (error) {
      console.error('Failed to create project:', error);
    } finally {
      creatingProject = false;
    }
  }

  async function handleDeleteProject(id: string) {
    if (confirm($_('dialogs.deleteProjectConfirm'))) {
      try {
        await api.deleteProject(id);
        removeProject(id);
      } catch (error) {
        console.error('Failed to delete project:', error);
      }
    }
  }

  function handleBrainstormComplete(project: ProjectState) {
    updateCurrentProject(project);
    showBrainstorm = false;
    resetLoop();
    clearLogs();
  }

  const availableCliCount = $derived($availableClis.filter(c => c.available).length);
</script>

<div class="h-screen flex flex-col bg-vscode-editor">
  <!-- Custom title bar (macOS overlay) -->
  <div
    class="h-8 flex items-center bg-vscode-editor border-b border-vscode px-4 pl-16 select-none"
    data-tauri-drag-region
  >
    <div class="flex-1" data-tauri-drag-region></div>
  </div>

  <div class="flex flex-1 min-h-0">
    <!-- Sidebar -->
    <div class="w-64 bg-vscode-sidebar border-r border-vscode flex flex-col">
    <!-- Header -->
    <div class="px-4 py-3 flex items-center justify-between">
      <div>
        <h1 class="text-sm font-semibold text-vscode uppercase tracking-wide">{$_('app.name')}</h1>
      </div>
      <button
        class="p-1.5 text-vscode-dim hover:text-vscode hover:bg-vscode-hover rounded"
        onclick={() => showSettings = true}
        title={$_('app.settings')}
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"/>
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"/>
        </svg>
      </button>
    </div>

    <!-- New Project Button -->
    <div class="px-3 pb-2">
      <button
        class="w-full py-1.5 px-3 bg-vscode-accent hover:bg-vscode-accent-hover text-white text-sm rounded flex items-center justify-center gap-2 disabled:opacity-50"
        onclick={handleCreateProject}
        disabled={creatingProject || availableCliCount === 0}
        data-testid="new-project"
      >
        <span>+</span>
        <span>{$_('app.newProject')}</span>
      </button>
      {#if availableCliCount === 0}
        <p class="text-xs text-[#f14c4c] mt-2 text-center">{$_('app.noCliDetected')}</p>
      {/if}
    </div>

    <!-- Project List -->
    <div class="flex-1 overflow-y-auto">
      <ProjectList
        projects={$projects}
        selectedId={$currentProjectId}
        onSelect={selectProject}
        onDelete={handleDeleteProject}
      />
    </div>

    <!-- Status Bar -->
    <QueueStatus />
    <div class="px-3 py-2 border-t border-vscode text-xs text-vscode-muted">
      <div class="flex justify-between">
        <span>{$availableClis.find(c => c.available)?.name || $_('app.noCli')}</span>
        <span>{$_('app.projectsCount', { values: { count: $projects.length } })}</span>
      </div>
    </div>
  </div>

    <!-- Main Content -->
    <div class="flex-1 flex flex-col overflow-hidden bg-vscode-editor">
    {#if availableCliCount === 0}
      <!-- No CLI Installed -->
      <CliNotInstalled clis={$availableClis} />
    {:else if $currentProject}
      {#if showBrainstorm}
        <AiBrainstorm
          project={$currentProject}
          onComplete={handleBrainstormComplete}
          onCancel={() => showBrainstorm = false}
        />
      {:else}
        <TaskDetail
          project={$currentProject}
          loopState={$loopState}
        />
      {/if}
    {:else}
      <!-- Empty State -->
      <div class="flex-1 flex items-center justify-center">
        <div class="text-center text-vscode-dim">
          <div class="text-5xl mb-4 opacity-30">📁</div>
          <h2 class="text-base font-medium mb-1 text-vscode">{$_('app.selectOrCreate')}</h2>
          <p class="text-sm text-vscode-muted">{$_('app.getStarted')}</p>
        </div>
      </div>
    {/if}
    </div>
  </div>
</div>

<!-- Settings Panel -->
{#if showSettings}
  <SettingsPanel onClose={() => showSettings = false} />
{/if}

<!-- Shortcuts Help -->
<ShortcutsHelp show={showShortcuts} onClose={() => showShortcuts = false} />

<!-- Keyboard Shortcuts Handler -->
<KeyboardShortcuts {shortcuts} />
