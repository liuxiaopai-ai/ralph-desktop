<script lang="ts">
  import { projects, currentProjectId, currentProject, selectProject, addProject, removeProject, updateCurrentProject } from '$lib/stores/projects';
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

  let showBrainstorm = $state(false);
  let showSettings = $state(false);
  let showShortcuts = $state(false);
  let creatingProject = $state(false);

  // Keyboard shortcuts
  const shortcuts = [
    { key: 'n', ctrl: true, action: handleCreateProject, description: '新建项目' },
    { key: ',', ctrl: true, action: () => showSettings = true, description: '打开设置' },
    { key: '?', ctrl: true, action: () => showShortcuts = true, description: '显示快捷键' },
    { key: 'Escape', action: handleEscape, description: '关闭对话框' },
  ];

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
      // Use Tauri dialog to select directory
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        directory: true,
        multiple: false,
        title: '选择项目目录'
      });

      if (selected) {
        const path = selected as string;
        const name = path.split('/').pop() || 'New Project';
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
    if (confirm('确定要删除这个项目吗？')) {
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

<div class="flex h-screen bg-gray-100 dark:bg-gray-900">
  <!-- Sidebar -->
  <div class="w-72 bg-white dark:bg-gray-800 border-r border-gray-200 dark:border-gray-700 flex flex-col">
    <!-- Header -->
    <div class="p-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
      <div>
        <h1 class="text-xl font-bold text-gray-800 dark:text-white">Ralph Desktop</h1>
        <p class="text-xs text-gray-500 dark:text-gray-400 mt-1">Visual Ralph Loop Controller</p>
      </div>
      <button
        class="p-2 text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg"
        onclick={() => showSettings = true}
        title="设置"
      >
        ⚙️
      </button>
    </div>

    <!-- New Project Button -->
    <div class="p-3">
      <button
        class="w-full py-2 px-4 bg-blue-600 hover:bg-blue-700 text-white rounded-lg flex items-center justify-center gap-2 disabled:opacity-50"
        onclick={handleCreateProject}
        disabled={creatingProject || availableCliCount === 0}
      >
        <span class="text-lg">+</span>
        <span>New Project</span>
      </button>
      {#if availableCliCount === 0}
        <p class="text-xs text-red-500 mt-2 text-center">未检测到 CLI，请先安装 Claude Code 或 Codex</p>
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
    <div class="p-3 border-t border-gray-200 dark:border-gray-700 text-xs text-gray-500 dark:text-gray-400">
      <div class="flex justify-between">
        <span>CLI: {$availableClis.find(c => c.available)?.name || 'None'}</span>
        <span>Projects: {$projects.length}</span>
      </div>
    </div>
  </div>

  <!-- Main Content -->
  <div class="flex-1 flex flex-col overflow-hidden">
    {#if $currentProject}
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
        <div class="text-center text-gray-500 dark:text-gray-400">
          <div class="text-6xl mb-4">📁</div>
          <h2 class="text-xl font-medium mb-2">选择或创建一个项目</h2>
          <p class="text-sm">点击左侧 "New Project" 开始</p>
        </div>
      </div>
    {/if}
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
