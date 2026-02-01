import { writable } from 'svelte/store';
import type { LogEntry, ProjectStatus } from '../types';

export interface LoopStoreState {
  status: ProjectStatus;
  currentIteration: number;
  maxIterations: number;
  logs: LogEntry[];
  lastError: string | null;
  startedAt: Date | null;
  endedAt: Date | null;
  elapsedMs: number | null;
  summary: string | null;
  summaryUpdatedAt: Date | null;
}

const createInitialState = (): LoopStoreState => ({
  status: 'ready',
  currentIteration: 0,
  maxIterations: 50,
  logs: [],
  lastError: null,
  startedAt: null,
  endedAt: null,
  elapsedMs: null,
  summary: null,
  summaryUpdatedAt: null
});

export const loopStates = writable<Record<string, LoopStoreState>>({});

export function getLoopState(
  states: Record<string, LoopStoreState>,
  projectId: string | null | undefined
): LoopStoreState {
  if (!projectId) {
    return createInitialState();
  }
  return states[projectId] ?? createInitialState();
}

function updateProjectState(
  projectId: string,
  updater: (state: LoopStoreState) => LoopStoreState
) {
  loopStates.update(states => {
    const current = states[projectId] ?? createInitialState();
    const next = updater(current);
    return { ...states, [projectId]: next };
  });
}

// Actions
export function resetLoop(projectId: string) {
  updateProjectState(projectId, () => createInitialState());
}

export function setMaxIterations(projectId: string, max: number) {
  updateProjectState(projectId, state => ({ ...state, maxIterations: max }));
}

export function addLog(projectId: string, entry: LogEntry) {
  updateProjectState(projectId, state => ({
    ...state,
    logs: [...state.logs.slice(-999), entry] // Keep last 1000 logs
  }));
}

export function setStatus(projectId: string, status: ProjectStatus) {
  updateProjectState(projectId, state => ({ ...state, status }));
}

export function setIteration(projectId: string, iteration: number) {
  updateProjectState(projectId, state => ({ ...state, currentIteration: iteration }));
}

export function setError(projectId: string, error: string | null) {
  updateProjectState(projectId, state => ({ ...state, lastError: error }));
}

export function markStarted(projectId: string, startedAt: Date = new Date()) {
  updateProjectState(projectId, state => ({
    ...state,
    startedAt,
    endedAt: null,
    elapsedMs: null,
    summary: null,
    summaryUpdatedAt: null,
    lastError: null
  }));
}

export function markEnded(projectId: string, endedAt: Date = new Date()) {
  updateProjectState(projectId, state => ({
    ...state,
    endedAt,
    elapsedMs: state.startedAt ? endedAt.getTime() - state.startedAt.getTime() : null
  }));
}

export function setSummary(
  projectId: string,
  summary: string | null,
  summaryUpdatedAt: Date = new Date()
) {
  updateProjectState(projectId, state => ({
    ...state,
    summary,
    summaryUpdatedAt: summary ? summaryUpdatedAt : null
  }));
}

export function clearLogs(projectId: string) {
  updateProjectState(projectId, state => ({ ...state, logs: [] }));
}

/**
 * Load loop state from persisted execution data (for restoring after app restart)
 */
export function loadFromExecution(
  projectId: string,
  status: ProjectStatus,
  currentIteration: number,
  maxIterations: number,
  elapsedMs: number | null,
  summary: string | null,
  startedAt: string | null,
  completedAt: string | null
) {
  updateProjectState(projectId, () => ({
    status,
    currentIteration,
    maxIterations,
    logs: [], // Logs will be loaded separately if needed
    lastError: null,
    startedAt: startedAt ? new Date(startedAt) : null,
    endedAt: completedAt ? new Date(completedAt) : null,
    elapsedMs,
    summary,
    summaryUpdatedAt: summary && completedAt ? new Date(completedAt) : null
  }));
}
