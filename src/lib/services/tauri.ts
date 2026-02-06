import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  GlobalConfig,
  ProjectMeta,
  ProjectState,
  CliInfo,
  CliType,
  ProjectStatus,
  LoopEvent,
  UpdateState
} from '../types';

const isE2E = import.meta.env.VITE_E2E === '1';
const e2eCli = (import.meta.env.VITE_E2E_CLI || 'codex') as CliType;

type LoopListener = (event: LoopEvent) => void;

type E2EProject = {
  state: ProjectState;
  logs: string[];
  isGitRepo: boolean;
  loop?: {
    iteration: number;
    maxIterations: number;
    paused: boolean;
    stopped: boolean;
    timer?: ReturnType<typeof setInterval>;
  };
};

const e2eState = (() => {
  const projects = new Map<string, E2EProject>();
  const listeners = new Set<LoopListener>();
  let config: GlobalConfig = {
    version: '0.1.1',
    defaultCli: e2eCli,
    defaultMaxIterations: 3,
    maxConcurrentProjects: 3,
    iterationTimeoutMs: 0,
    idleTimeoutMs: 0,
    theme: 'system',
    language: 'en',
    logRetentionDays: 7,
    permissionsConfirmed: true
  };

  function now() {
    return new Date().toISOString();
  }

  function emit(event: LoopEvent) {
    for (const listener of listeners) {
      listener(event);
    }
  }

  function createId() {
    if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
      return crypto.randomUUID();
    }
    return `e2e-${Date.now()}-${Math.random().toString(16).slice(2)}`;
  }

  function ensureProject(projectId: string) {
    const project = projects.get(projectId);
    if (!project) {
      throw new Error('Project not found');
    }
    return project;
  }

  function startLoop(projectId: string) {
    const project = ensureProject(projectId);
    if (!project.state.task) {
      throw new Error('No task configured for this project');
    }

    const maxIterations = project.state.task.maxIterations || config.defaultMaxIterations || 3;
    project.loop = {
      iteration: 0,
      maxIterations,
      paused: false,
      stopped: false
    };

    project.state.status = 'running';
    project.state.execution = {
      startedAt: now(),
      currentIteration: 0,
      lastOutput: ''
    };

    const completionSignal = project.state.task.completionSignal || '<done>COMPLETE</done>';

    project.loop.timer = setInterval(() => {
      const loop = project.loop;
      if (!loop || loop.stopped || loop.paused) {
        return;
      }

      loop.iteration += 1;
      project.state.execution!.currentIteration = loop.iteration;
      emit({ type: 'iterationStart', projectId, iteration: loop.iteration });

      const line = `iteration ${loop.iteration}: generating web build (HTML5 Canvas / JavaScript)`;
      project.logs.push(line);
      emit({ type: 'output', projectId, iteration: loop.iteration, content: line, isStderr: false });

      if (loop.iteration >= loop.maxIterations) {
        const doneLine = completionSignal;
        project.logs.push(doneLine);
        emit({ type: 'output', projectId, iteration: loop.iteration, content: doneLine, isStderr: false });
        emit({ type: 'completed', projectId, iteration: loop.iteration });
        project.state.status = 'done';
        project.state.execution!.completedAt = now();
        loop.stopped = true;
        if (loop.timer) {
          clearInterval(loop.timer);
        }
      }
    }, 700);
  }

  function pauseLoop(projectId: string) {
    const project = ensureProject(projectId);
    const loop = project.loop;
    if (!loop) {
      return;
    }
    emit({ type: 'pausing', projectId, iteration: loop.iteration });
    project.state.status = 'pausing';
    loop.paused = true;
    setTimeout(() => {
      emit({ type: 'paused', projectId, iteration: loop.iteration });
      project.state.status = 'paused';
    }, 200);
  }

  function resumeLoop(projectId: string) {
    const project = ensureProject(projectId);
    const loop = project.loop;
    if (!loop) {
      return;
    }
    loop.paused = false;
    emit({ type: 'resumed', projectId, iteration: loop.iteration });
    project.state.status = 'running';
  }

  function stopLoop(projectId: string) {
    const project = ensureProject(projectId);
    const loop = project.loop;
    if (!loop) {
      return;
    }
    loop.stopped = true;
    if (loop.timer) {
      clearInterval(loop.timer);
    }
    emit({ type: 'stopped', projectId });
    project.state.status = 'cancelled';
  }

  return {
    getConfig() {
      return config;
    },
    saveConfig(next: GlobalConfig) {
      config = { ...next };
    },
    listProjects() {
      return Array.from(projects.values()).map(({ state }) => ({
        id: state.id,
        name: state.name,
        path: state.path,
        status: state.status,
        createdAt: state.createdAt,
        lastOpenedAt: state.updatedAt
      }));
    },
    createProject(path: string, name: string) {
      const id = createId();
      const ts = now();
      const state: ProjectState = {
        id,
        name,
        path,
        status: 'brainstorming',
        skipGitRepoCheck: true,
        brainstorm: { answers: [] },
        task: undefined,
        execution: undefined,
        createdAt: ts,
        updatedAt: ts
      };
      projects.set(id, { state, logs: [], isGitRepo: false });
      return state;
    },
    getProject(id: string) {
      return ensureProject(id).state;
    },
    checkProjectGitRepo(projectId: string) {
      return ensureProject(projectId).isGitRepo;
    },
    initProjectGitRepo(projectId: string) {
      const project = ensureProject(projectId);
      project.isGitRepo = true;
      project.state.updatedAt = now();
    },
    setProjectSkipGitRepoCheck(projectId: string, skip: boolean) {
      const project = ensureProject(projectId);
      project.state.skipGitRepoCheck = skip;
      project.state.updatedAt = now();
      return project.state;
    },
    updateTaskMaxIterations(projectId: string, maxIterations: number) {
      const project = ensureProject(projectId);
      if (!project.state.task) {
        throw new Error('No task configured for this project');
      }
      project.state.task.maxIterations = maxIterations;
      project.state.updatedAt = now();
      return project.state;
    },
    updateTaskAutoCommit(projectId: string, autoCommit: boolean) {
      const project = ensureProject(projectId);
      if (!project.state.task) {
        throw new Error('No task configured for this project');
      }
      project.state.task.autoCommit = autoCommit;
      project.state.updatedAt = now();
      return project.state;
    },
    updateTaskAutoInit(projectId: string, autoInitGit: boolean) {
      const project = ensureProject(projectId);
      if (!project.state.task) {
        throw new Error('No task configured for this project');
      }
      project.state.task.autoInitGit = autoInitGit;
      project.state.updatedAt = now();
      return project.state;
    },
    updateTaskPrompt(projectId: string, prompt: string) {
      const project = ensureProject(projectId);
      if (!project.state.task) {
        throw new Error('No task configured for this project');
      }
      project.state.task.prompt = prompt;
      project.state.updatedAt = now();
      return project.state;
    },
    deleteProject(id: string) {
      projects.delete(id);
    },
    detectInstalledClis(): CliInfo[] {
      return [
        { cliType: 'codex', name: 'Codex CLI', version: 'mock', path: '/usr/local/bin/codex', available: true },
        { cliType: 'claude', name: 'Claude Code', version: 'mock', path: '/usr/local/bin/claude', available: true },
        { cliType: 'opencode', name: 'OpenCode', version: 'mock', path: '/usr/local/bin/opencode', available: false }
      ];
    },
    confirmPermissions() {
      config.permissionsConfirmed = true;
    },
    updateProjectStatus(projectId: string, status: ProjectStatus) {
      const project = ensureProject(projectId);
      project.state.status = status;
      project.state.updatedAt = now();
      return project.state;
    },
    startLoop(projectId: string) {
      startLoop(projectId);
    },
    pauseLoop(projectId: string) {
      pauseLoop(projectId);
    },
    resumeLoop(projectId: string) {
      resumeLoop(projectId);
    },
    stopLoop(projectId: string) {
      stopLoop(projectId);
    },
    getLoopStatus(projectId: string) {
      const project = ensureProject(projectId);
      return Boolean(project.loop && !project.loop.stopped);
    },
    checkInterruptedTasks() {
      return [] as RecoveryInfo[];
    },
    cancelInterruptedTask(projectId: string) {
      stopLoop(projectId);
    },
    cleanupLogs() {
      return 0;
    },
    getProjectLogs(projectId: string) {
      const project = ensureProject(projectId);
      return project.logs;
    },
    aiBrainstormChat(projectId: string, conversation: ConversationMessage[]) {
      const topic = conversation[conversation.length - 1]?.content || 'task';
      const prompt = [
        `Task: Build a unique ${topic} game`,
        `Tech: HTML5 Canvas / JavaScript`,
        `Requirements: smooth controls, clear UI, interesting twist`,
        `Testing & Validation: include unit tests for core logic; add a minimal E2E smoke test if UI flow exists`,
        `Test Commands: npm test (unit), npm run e2e (if available)`,
        `Success: playable in browser`,
        `<done>COMPLETE</done>`
      ].join('\n');

      return {
        question: 'Thanks! Preparing your task...',
        description: 'Auto-generated for E2E testing',
        options: [],
        multiSelect: false,
        allowOther: false,
        isComplete: true,
        generatedPrompt: prompt
      } as AiBrainstormResponse;
    },
    completeAiBrainstorm(projectId: string, generatedPrompt: string, cli: CliType, maxIterations: number) {
      const project = ensureProject(projectId);
      project.state.task = {
        prompt: generatedPrompt,
        cli,
        maxIterations,
        autoCommit: true,
        autoInitGit: true,
        completionSignal: '<done>COMPLETE</done>'
      };
      project.state.status = 'ready';
      project.state.updatedAt = now();
      return project.state;
    },
    listenToLoopEvents(callback: LoopListener) {
      listeners.add(callback);
      return () => listeners.delete(callback);
    },
    getUpdateState() {
      const state: UpdateState = {
        status: 'idle',
        currentVersion: '0.0.0',
        targetVersion: null,
        lastCheckedAt: null,
        failureCount: 0,
        lastError: null,
        downloadPath: null,
        sha256: null,
        pending: false
      };
      return state;
    },
    checkForUpdates() {
      return this.getUpdateState();
    },
    loadUpdateState() {
      return this.getUpdateState();
    }
  };
})();

// Project Commands
export async function listProjects(): Promise<ProjectMeta[]> {
  if (isE2E) return e2eState.listProjects();
  return invoke('list_projects');
}

export async function createProject(path: string, name: string): Promise<ProjectState> {
  if (isE2E) return e2eState.createProject(path, name);
  return invoke('create_project', { path, name });
}

export async function getProject(id: string): Promise<ProjectState> {
  if (isE2E) return e2eState.getProject(id);
  return invoke('get_project', { id });
}

export async function setProjectSkipGitRepoCheck(
  projectId: string,
  skip: boolean
): Promise<ProjectState> {
  if (isE2E) return e2eState.setProjectSkipGitRepoCheck(projectId, skip);
  return invoke('set_project_skip_git_repo_check', { projectId, skip });
}

export async function updateTaskMaxIterations(
  projectId: string,
  maxIterations: number
): Promise<ProjectState> {
  if (isE2E) return e2eState.updateTaskMaxIterations(projectId, maxIterations);
  return invoke('update_task_max_iterations', { projectId, maxIterations });
}

export async function updateTaskAutoCommit(
  projectId: string,
  autoCommit: boolean
): Promise<ProjectState> {
  if (isE2E) return e2eState.updateTaskAutoCommit(projectId, autoCommit);
  return invoke('update_task_auto_commit', { projectId, autoCommit });
}

export async function updateTaskAutoInit(
  projectId: string,
  autoInitGit: boolean
): Promise<ProjectState> {
  if (isE2E) return e2eState.updateTaskAutoInit(projectId, autoInitGit);
  return invoke('update_task_auto_init', { projectId, autoInitGit });
}

export async function updateTaskPrompt(
  projectId: string,
  prompt: string
): Promise<ProjectState> {
  if (isE2E) return e2eState.updateTaskPrompt(projectId, prompt);
  return invoke('update_task_prompt', { projectId, prompt });
}

export async function initProjectGitRepo(projectId: string): Promise<void> {
  if (isE2E) return e2eState.initProjectGitRepo(projectId);
  return invoke('init_project_git_repo', { projectId });
}

export async function checkProjectGitRepo(projectId: string): Promise<boolean> {
  if (isE2E) return e2eState.checkProjectGitRepo(projectId);
  return invoke('check_project_git_repo', { projectId });
}

export async function deleteProject(id: string): Promise<void> {
  if (isE2E) return e2eState.deleteProject(id);
  return invoke('delete_project', { id });
}

// CLI Commands
export async function detectInstalledClis(): Promise<CliInfo[]> {
  if (isE2E) return e2eState.detectInstalledClis();
  return invoke('detect_installed_clis');
}

// Config Commands
export async function getConfig(): Promise<GlobalConfig> {
  if (isE2E) return e2eState.getConfig();
  return invoke('get_config');
}

export async function saveConfig(config: GlobalConfig): Promise<void> {
  if (isE2E) return e2eState.saveConfig(config);
  return invoke('save_config', { config });
}

export async function confirmPermissions(): Promise<void> {
  if (isE2E) return e2eState.confirmPermissions();
  return invoke('confirm_permissions');
}

export async function updateProjectStatus(
  projectId: string,
  status: ProjectStatus
): Promise<ProjectState> {
  if (isE2E) return e2eState.updateProjectStatus(projectId, status);
  return invoke('update_project_status', { projectId, status });
}

// Loop Commands
export async function startLoop(projectId: string): Promise<void> {
  if (isE2E) return e2eState.startLoop(projectId);
  return invoke('start_loop', { projectId });
}

export async function pauseLoop(projectId: string): Promise<void> {
  if (isE2E) return e2eState.pauseLoop(projectId);
  return invoke('pause_loop', { projectId });
}

export async function resumeLoop(projectId: string): Promise<void> {
  if (isE2E) return e2eState.resumeLoop(projectId);
  return invoke('resume_loop', { projectId });
}

export async function stopLoop(projectId: string): Promise<void> {
  if (isE2E) return e2eState.stopLoop(projectId);
  return invoke('stop_loop', { projectId });
}

export async function getLoopStatus(projectId: string): Promise<boolean> {
  if (isE2E) return e2eState.getLoopStatus(projectId);
  return invoke('get_loop_status', { projectId });
}

// Recovery Commands
export interface RecoveryInfo {
  projectId: string;
  projectName: string;
  iteration: number;
  status: string;
}

export async function checkInterruptedTasks(): Promise<RecoveryInfo[]> {
  if (isE2E) return e2eState.checkInterruptedTasks();
  return invoke('check_interrupted_tasks');
}

export async function cancelInterruptedTask(projectId: string): Promise<void> {
  if (isE2E) return e2eState.cancelInterruptedTask(projectId);
  return invoke('cancel_interrupted_task', { projectId });
}

export async function cleanupLogs(): Promise<number> {
  if (isE2E) return e2eState.cleanupLogs();
  return invoke('cleanup_logs');
}

export async function getProjectLogs(projectId: string): Promise<string[]> {
  if (isE2E) return e2eState.getProjectLogs(projectId);
  return invoke('get_project_logs', { projectId });
}

// Update Commands
export async function getUpdateState(): Promise<UpdateState> {
  if (isE2E) return e2eState.getUpdateState();
  return invoke('get_update_state');
}

export async function checkForUpdates(idleOk: boolean): Promise<UpdateState> {
  if (isE2E) return e2eState.checkForUpdates();
  return invoke('check_for_updates', { idleOk });
}

export async function loadUpdateState(): Promise<UpdateState> {
  if (isE2E) return e2eState.loadUpdateState();
  return invoke('load_update_state_cmd');
}

// AI Brainstorm Types
export interface ConversationMessage {
  role: 'user' | 'assistant';
  content: string;
}

export interface QuestionOption {
  label: string;
  description?: string;
  value: string;
}

export interface AiBrainstormResponse {
  question: string;
  description?: string;
  options: QuestionOption[];
  multiSelect: boolean;
  allowOther: boolean;
  isComplete: boolean;
  generatedPrompt?: string;
}

// AI Brainstorm Commands
export async function aiBrainstormChat(
  projectId: string,
  conversation: ConversationMessage[]
): Promise<AiBrainstormResponse> {
  if (isE2E) return e2eState.aiBrainstormChat(projectId, conversation);
  return invoke('ai_brainstorm_chat', { projectId, conversation });
}

export async function completeAiBrainstorm(
  projectId: string,
  generatedPrompt: string,
  cli: CliType,
  maxIterations: number
): Promise<ProjectState> {
  if (isE2E) return e2eState.completeAiBrainstorm(projectId, generatedPrompt, cli, maxIterations);
  return invoke('complete_ai_brainstorm', { projectId, generatedPrompt, cli, maxIterations });
}

// Event Listeners
export async function listenToLoopEvents(
  callback: (event: LoopEvent) => void
): Promise<UnlistenFn> {
  if (isE2E) {
    return Promise.resolve(e2eState.listenToLoopEvents(callback));
  }
  return listen<LoopEvent>('loop-event', (event) => {
    callback(event.payload);
  });
}
