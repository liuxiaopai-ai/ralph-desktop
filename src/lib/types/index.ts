// CLI Types
export type CliType = 'claude' | 'codex';
export type Theme = 'light' | 'dark' | 'system';
export type ProjectStatus = 'brainstorming' | 'ready' | 'queued' | 'running' | 'pausing' | 'paused' | 'done' | 'failed' | 'cancelled';

// Global Config
export interface GlobalConfig {
  version: string;
  defaultCli: CliType;
  defaultMaxIterations: number;
  maxConcurrentProjects: number;
  iterationTimeoutMs: number;
  idleTimeoutMs: number;
  theme: Theme;
  logRetentionDays: number;
  permissionsConfirmed: boolean;
  permissionsConfirmedAt?: string;
}

// Project Types
export interface ProjectMeta {
  id: string;
  name: string;
  path: string;
  status: ProjectStatus;
  createdAt: string;
  lastOpenedAt: string;
}

export interface ProjectState {
  id: string;
  name: string;
  path: string;
  status: ProjectStatus;
  skipGitRepoCheck?: boolean;
  brainstorm?: BrainstormState;
  task?: TaskConfig;
  execution?: ExecutionState;
  createdAt: string;
  updatedAt: string;
}

export interface BrainstormState {
  answers: BrainstormAnswer[];
  completedAt?: string;
}

export interface BrainstormAnswer {
  questionId: string;
  question: string;
  answer: string | string[];
  answeredAt: string;
}

export interface TaskConfig {
  prompt: string;
  designDocPath?: string;
  cli: CliType;
  maxIterations: number;
  completionSignal: string;
}

export interface ExecutionState {
  startedAt: string;
  pausedAt?: string;
  completedAt?: string;
  currentIteration: number;
  lastOutput: string;
  lastError?: string;
  lastExitCode?: number;
}

// CLI Info
export interface CliInfo {
  cliType: CliType;
  name: string;
  version?: string;
  path: string;
  available: boolean;
}

// Loop Events
export type LoopEventType =
  | 'iterationStart'
  | 'output'
  | 'pausing'
  | 'paused'
  | 'resumed'
  | 'completed'
  | 'maxIterationsReached'
  | 'error'
  | 'stopped';

export interface LoopEvent {
  type: LoopEventType;
  projectId: string;
  iteration?: number;
  content?: string;
  isStderr?: boolean;
  error?: string;
}

// Log Entry
export interface LogEntry {
  iteration: number;
  content: string;
  isStderr: boolean;
  timestamp: Date;
}
