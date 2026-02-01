import { writable, get } from 'svelte/store';
import type { GlobalConfig, CliInfo } from '../types';

const defaultConfig: GlobalConfig = {
  version: '1.0.0',
  defaultCli: 'claude',
  defaultMaxIterations: 50,
  maxConcurrentProjects: 3,
  iterationTimeoutMs: 0,
  idleTimeoutMs: 0,
  theme: 'system',
  language: 'system',
  logRetentionDays: 7,
  permissionsConfirmed: false
};

export const config = writable<GlobalConfig>(defaultConfig);
export const availableClis = writable<CliInfo[]>([]);

export function updateConfig(newConfig: GlobalConfig) {
  config.set(newConfig);
}

export function setAvailableClis(clis: CliInfo[]) {
  availableClis.set(clis);
  // Also verify if current defaultCli is actually available
  // Check if current defaultCli is available
  const current = get(config);
  if (clis.length > 0 && !clis.find(cli => cli.cliType === current.defaultCli)) {
    // Logic to update default could go here if desired, but for now we just keeping it safe
  }
}
