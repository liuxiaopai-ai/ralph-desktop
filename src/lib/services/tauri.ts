import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  GlobalConfig,
  ProjectMeta,
  ProjectState,
  CliInfo,
  QuestionTemplate,
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

// Brainstorm Commands
export async function getBrainstormQuestions(): Promise<QuestionTemplate[]> {
  return invoke('get_brainstorm_questions');
}

export async function saveBrainstormAnswer(
  projectId: string,
  questionId: string,
  question: string,
  answer: string | string[]
): Promise<ProjectState> {
  return invoke('save_brainstorm_answer', { projectId, questionId, question, answer });
}

export async function completeBrainstorm(
  projectId: string,
  cli: CliType,
  maxIterations: number
): Promise<ProjectState> {
  return invoke('complete_brainstorm', { projectId, cli, maxIterations });
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

export interface AiBrainstormResponse {
  message: string;
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
