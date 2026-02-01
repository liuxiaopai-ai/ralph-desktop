// CLI Types
export type CliType = 'claude' | 'codex' | 'opencode';
export type Theme = 'light' | 'dark' | 'system';

export type UpdateStatus =
  | 'idle'
  | 'checking'
  | 'no_update'
  | 'update_available'
  | 'waiting_for_idle'
  | 'downloading'
  | 'verifying'
  | 'self_test'
  | 'ready_to_apply'
  | 'applied_on_next_launch'
  | 'failed';

export interface UpdateState {
  status: UpdateStatus;
  currentVersion: string;
  targetVersion?: string | null;
  lastCheckedAt?: string | null;
  failureCount: number;
  lastError?: string | null;
  downloadPath?: string | null;
  sha256?: string | null;
  pending: boolean;
}
export type ProjectStatus =
  | 'brainstorming'
  | 'ready'
  | 'queued'
  | 'running'
  | 'pausing'
  | 'paused'
  | 'done'
  | 'partial'
  | 'failed'
  | 'cancelled';

// Global Config
export interface GlobalConfig {
  version: string;
  defaultCli: CliType;
  defaultMaxIterations: number;
  maxConcurrentProjects: number;
  iterationTimeoutMs: number;
  idleTimeoutMs: number;
  theme: Theme;
  language: string;
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
  /** Legacy fields for backward compatibility */
  brainstorm?: BrainstormState;
  task?: TaskConfig;
  execution?: ExecutionState;
  /** Multiple sessions/conversations for this project */
  sessions?: Session[];
  /** Currently active session ID */
  activeSessionId?: string;
  createdAt: string;
  updatedAt: string;
}

export interface Session {
  id: string;
  name: string;
  status: ProjectStatus;
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
  autoCommit?: boolean;
  autoInitGit?: boolean;
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
  /** Persisted elapsed time in milliseconds */
  elapsedMs?: number;
  /** Persisted task summary */
  summary?: string;
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
