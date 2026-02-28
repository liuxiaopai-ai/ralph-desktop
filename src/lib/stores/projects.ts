import { writable, derived } from 'svelte/store';
import type { ProjectMeta, ProjectState, ProjectStatus } from '../types';

// Project list
export const projects = writable<ProjectMeta[]>([]);

// Current project ID
export const currentProjectId = writable<string | null>(null);

// Current project state (full details)
export const currentProject = writable<ProjectState | null>(null);

// Derived: is any project selected
export const hasSelectedProject = derived(
  currentProjectId,
  ($id) => $id !== null
);

// Actions
export function selectProject(id: string | null) {
  currentProjectId.set(id);
}

export function updateProjects(newProjects: ProjectMeta[]) {
  projects.set(newProjects);
}

export function addProject(project: ProjectMeta) {
  projects.update(list => [...list, project]);
}

export function removeProject(id: string) {
  projects.update(list => list.filter(p => p.id !== id));
  currentProjectId.update(current => current === id ? null : current);
}

export function updateCurrentProject(state: ProjectState) {
  currentProject.set(state);
}

export function updateProjectName(id: string, name: string) {
  projects.update(list => list.map(p => p.id === id ? { ...p, name } : p));
  currentProject.update(current => {
    if (!current || current.id !== id) return current;
    return { ...current, name };
  });
}

export function updateProjectStatus(id: string, status: ProjectStatus) {
  projects.update(list => list.map(p => p.id === id ? { ...p, status } : p));
  currentProject.update(current => {
    if (!current || current.id !== id) return current;
    return { ...current, status };
  });
}
