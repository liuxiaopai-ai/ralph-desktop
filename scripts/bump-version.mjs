#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';

const version = process.argv[2];
if (!version || !/^\d+\.\d+\.\d+$/.test(version)) {
  console.error('Usage: node scripts/bump-version.mjs <MAJOR.MINOR.PATCH>');
  process.exit(1);
}

const root = process.cwd();

function updateJson(filePath, updater) {
  const raw = fs.readFileSync(filePath, 'utf8');
  const data = JSON.parse(raw);
  const next = updater(data);
  fs.writeFileSync(filePath, JSON.stringify(next, null, 2) + '\n');
}

function updateText(filePath, replacer) {
  const raw = fs.readFileSync(filePath, 'utf8');
  const next = replacer(raw);
  fs.writeFileSync(filePath, next);
}

updateJson(path.join(root, 'package.json'), (data) => ({
  ...data,
  version,
}));

updateText(path.join(root, 'src-tauri', 'Cargo.toml'), (raw) => {
  const next = raw.replace(/^(version\s*=\s*")([^"]+)(")/m, `$1${version}$3`);
  if (next === raw) {
    throw new Error('Cargo.toml version not found');
  }
  return next;
});

updateJson(path.join(root, 'src-tauri', 'tauri.conf.json'), (data) => ({
  ...data,
  version,
}));

console.log(`Version bumped to ${version}`);
