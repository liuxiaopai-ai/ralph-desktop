import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  GlobalConfig,
  ProjectMeta,
  ProjectState,
  CliInfo,
  CliType,
  ProjectStatus,
  LoopEvent
} from '../types';

// Project Commands
export async function listProjects(): Promise<ProjectMeta[]> {
  return invoke('list_projects');
}

export async function createProject(path: string, name: string): Promise<ProjectState> {
  return invoke('create_project', { path, name });
}

export async function getProject(id: string): Promise<ProjectState> {
  return invoke('get_project', { id });
}

export async function setProjectSkipGitRepoCheck(
  projectId: string,
  skip: boolean
): Promise<ProjectState> {
  return invoke('set_project_skip_git_repo_check', { projectId, skip });
}

export async function initProjectGitRepo(projectId: string): Promise<void> {
  return invoke('init_project_git_repo', { projectId });
}

export async function deleteProject(id: string): Promise<void> {
  return invoke('delete_project', { id });
}

// CLI Commands
export async function detectInstalledClis(): Promise<CliInfo[]> {
  return invoke('detect_installed_clis');
}

// Config Commands
export async function getConfig(): Promise<GlobalConfig> {
  return invoke('get_config');
}

export async function saveConfig(config: GlobalConfig): Promise<void> {
  return invoke('save_config', { config });
}

export async function confirmPermissions(): Promise<void> {
  return invoke('confirm_permissions');
}

export async function updateProjectStatus(
  projectId: string,
  status: ProjectStatus
): Promise<ProjectState> {
  return invoke('update_project_status', { projectId, status });
}

// Loop Commands
export async function startLoop(projectId: string): Promise<void> {
  return invoke('start_loop', { projectId });
}

export async function pauseLoop(projectId: string): Promise<void> {
  return invoke('pause_loop', { projectId });
}

export async function resumeLoop(projectId: string): Promise<void> {
  return invoke('resume_loop', { projectId });
}

export async function stopLoop(projectId: string): Promise<void> {
  return invoke('stop_loop', { projectId });
}

export async function getLoopStatus(projectId: string): Promise<boolean> {
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
  return invoke('check_interrupted_tasks');
}

export async function cancelInterruptedTask(projectId: string): Promise<void> {
  return invoke('cancel_interrupted_task', { projectId });
}

export async function cleanupLogs(): Promise<number> {
  return invoke('cleanup_logs');
}

export async function getProjectLogs(projectId: string): Promise<string[]> {
  return invoke('get_project_logs', { projectId });
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
  return invoke('ai_brainstorm_chat', { projectId, conversation });
}

export async function completeAiBrainstorm(
  projectId: string,
  generatedPrompt: string,
  cli: CliType,
  maxIterations: number
): Promise<ProjectState> {
  return invoke('complete_ai_brainstorm', { projectId, generatedPrompt, cli, maxIterations });
}

// Event Listeners
export async function listenToLoopEvents(
  callback: (event: LoopEvent) => void
): Promise<UnlistenFn> {
  return listen<LoopEvent>('loop-event', (event) => {
    callback(event.payload);
  });
}
