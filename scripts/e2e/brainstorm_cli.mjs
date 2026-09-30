import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import net from 'node:net';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';

// Exercise the production service functions. Only the desktop IPC boundary is
// mocked: VITE_E2E's in-memory service would hide missing invoke arguments.
const HOST = process.env.E2E_HOST || '127.0.0.1';
const CLIS = ['claude', 'codex', 'opencode'];
const GENERATED_PROMPT = 'Build an offline notes app with synchronization.';
const ROOT = fileURLToPath(new URL('../../', import.meta.url));

async function availablePort() {
  const server = net.createServer();
  server.listen(0, HOST);
  await once(server, 'listening');
  const { port } = server.address();
  await new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve()));
  return port;
}

async function waitForServer(url, child, output) {
  const deadline = Date.now() + 60000;
  while (Date.now() < deadline) {
    if (child.exitCode !== null) {
      throw new Error(`Vite exited with ${child.exitCode}:\n${output()}`);
    }
    try {
      const response = await fetch(url);
      if (response.ok) return;
    } catch {
      // Vite has not started listening yet.
    }
    await delay(200);
  }
  throw new Error(`Timed out waiting for Vite:\n${output()}`);
}

function installDesktopMock(defaultCli) {
  const calls = [];
  const unexpected = [];
  const projects = new Map();
  let pending;
  let nextCallback = 0;
  const config = {
    version: '0.1.17', defaultCli, defaultMaxIterations: 7,
    maxConcurrentProjects: 3, iterationTimeoutMs: 0, idleTimeoutMs: 0,
    theme: 'dark', language: 'en', logRetentionDays: 7, permissionsConfirmed: true
  };
  const updateState = { status: 'idle', currentVersion: '0.1.17', pending: false };

  window.__brainstormTest = {
    calls,
    unexpected,
    config,
    resolve(response) {
      if (!pending) throw new Error('No brainstorm request is pending');
      pending.resolve(response);
      pending = undefined;
    },
    reject() {
      if (!pending) throw new Error('No brainstorm request is pending');
      pending.reject(new Error('Simulated CLI failure'));
      pending = undefined;
    }
  };
  window.__TAURI_INTERNALS__ = {
    transformCallback: () => ++nextCallback,
    async invoke(command, args = {}) {
      // Svelte state is proxied; IPC serializes it rather than cloning proxies.
      calls.push({ command, args: JSON.parse(JSON.stringify(args)) });
      switch (command) {
        case 'get_config': return structuredClone(config);
        case 'detect_installed_clis':
          return ['claude', 'codex', 'opencode'].map(cliType => ({
            cliType, name: cliType, version: 'test', path: `/test/${cliType}`, available: true
          }));
        case 'list_projects':
        case 'check_interrupted_tasks':
        case 'get_project_logs': return [];
        case 'plugin:event|listen': return ++nextCallback;
        case 'plugin:app|set_app_theme': return;
        case 'cleanup_logs': return 0;
        case 'load_update_state_cmd':
        case 'check_for_updates': return updateState;
        case 'plugin:dialog|open': return '/tmp/ralph-e2e-brainstorm-cli';
        case 'create_project': {
          const project = {
            id: `project-${projects.size + 1}`, name: args.name, path: args.path,
            status: 'brainstorming', brainstorm: { answers: [] },
            createdAt: new Date().toISOString(), updatedAt: new Date().toISOString()
          };
          projects.set(project.id, project);
          return structuredClone(project);
        }
        case 'get_project': return structuredClone(projects.get(args.id));
        case 'generate_project_title_cmd': return 'Notes app';
        case 'ai_brainstorm_chat':
          if (pending) throw new Error('Overlapping brainstorm requests');
          return new Promise((resolve, reject) => { pending = { resolve, reject }; });
        case 'complete_ai_brainstorm': {
          const project = projects.get(args.projectId);
          project.status = 'ready';
          project.task = {
            prompt: args.generatedPrompt, cli: args.cli, maxIterations: args.maxIterations,
            autoCommit: true, autoInitGit: true, completionSignal: '<done>COMPLETE</done>'
          };
          return structuredClone(project);
        }
        case 'check_project_git_repo': return true;
        default:
          unexpected.push(command);
          throw new Error(`Unexpected IPC command: ${command}`);
      }
    }
  };
}

async function callsFor(page, command) {
  return page.evaluate(command => window.__brainstormTest.calls.filter(call => call.command === command), command);
}

async function expectRequest(page, count, cli, content) {
  await page.waitForFunction(count =>
    window.__brainstormTest.calls.filter(call => call.command === 'ai_brainstorm_chat').length === count,
  count);
  const calls = await callsFor(page, 'ai_brainstorm_chat');
  assert.equal(calls.length, count, 'one request per submission');
  assert.equal(calls.at(-1).args.cli, cli, 'the selected CLI must reach the production IPC request');
  assert.equal(calls.at(-1).args.projectId, 'project-1');
  assert.deepEqual(calls.at(-1).args.conversation.at(-1), { role: 'user', content });
  await page.waitForFunction(() => document.querySelector('[data-testid="brainstorm-cli"]')?.disabled);
  assert.equal(await page.getByTestId('brainstorm-cli').inputValue(), cli);
  return calls.at(-1).args.conversation;
}

async function respond(page, response = {}) {
  await page.evaluate(response => window.__brainstormTest.resolve({
    question: 'Tell me more', options: [], multiSelect: false,
    allowOther: false, isComplete: false, ...response
  }), response);
  await page.waitForFunction(() => {
    const select = document.querySelector('[data-testid="brainstorm-cli"]');
    return select && !select.disabled;
  });
}

async function runScenario(browser, url, defaultCli, firstCli = defaultCli) {
  const context = await browser.newContext();
  await context.addInitScript(installDesktopMock, defaultCli);
  const page = await context.newPage();
  page.setDefaultTimeout(30000);
  const pageErrors = [];
  const browserErrors = [];
  page.on('pageerror', error => pageErrors.push(error.message));
  page.on('console', message => {
    if (message.type() === 'error') browserErrors.push(message.text());
  });
  try {
    await page.goto(url);
    await page.getByTestId('new-project').click();
    const selector = page.getByTestId('brainstorm-cli');
    await selector.waitFor();
    assert.equal(await selector.count(), 1, 'there is one shared CLI selector');
    assert.equal(await selector.inputValue(), defaultCli, 'new sessions inherit the global default');
    assert.deepEqual(await selector.locator('option').evaluateAll(options => options.map(option => option.value)), CLIS);
    await selector.selectOption(firstCli);

    await page.getByTestId('brainstorm-input').fill('Build a notes app');
    await page.getByTestId('brainstorm-input').press('Enter');
    await expectRequest(page, 1, firstCli, 'Build a notes app');
    assert.deepEqual(await callsFor(page, 'generate_project_title_cmd'), [{
      command: 'generate_project_title_cmd',
      args: { projectId: 'project-1', firstMessage: 'Build a notes app', cli: firstCli }
    }], 'title generation uses the same first-request CLI');

    const secondCli = CLIS[(CLIS.indexOf(firstCli) + 1) % CLIS.length];
    const thirdCli = CLIS[(CLIS.indexOf(firstCli) + 2) % CLIS.length];
    await respond(page, { question: 'Choose a platform', options: [{ label: 'Web app', value: 'web' }] });
    await selector.selectOption(secondCli);
    await page.getByRole('button', { name: 'Web app', exact: true }).click();
    const singleConversation = await expectRequest(page, 2, secondCli, 'Web app');
    assert.deepEqual(singleConversation, [
      { role: 'user', content: 'Build a notes app' },
      { role: 'assistant', content: 'Choose a platform' },
      { role: 'user', content: 'Web app' }
    ], 'switching CLI preserves the conversation');

    await respond(page, {
      question: 'Choose features', multiSelect: true, allowOther: true,
      options: [{ label: 'Offline support', value: 'offline' }, { label: 'Cloud sync', value: 'sync' }]
    });
    await selector.selectOption(thirdCli);
    await page.getByRole('button', { name: 'Offline support', exact: true }).click();
    await page.getByRole('button', { name: 'Cloud sync', exact: true }).click();
    await page.getByRole('button', { name: 'Other', exact: true }).click();
    await page.getByTestId('brainstorm-input').fill('encrypted');
    assert.equal((await callsFor(page, 'ai_brainstorm_chat')).length, 2, 'multi-select waits for submission');
    await page.getByTestId('brainstorm-submit').click();
    await expectRequest(page, 3, thirdCli, 'offline, sync, encrypted');

    await page.evaluate(() => window.__brainstormTest.reject());
    await page.getByText('Simulated CLI failure', { exact: true }).waitFor();
    assert.equal(await selector.isEnabled(), true, 'a failed request unlocks CLI selection');
    assert.equal(await selector.inputValue(), thirdCli);
    await selector.selectOption(secondCli);
    await page.getByTestId('brainstorm-input').fill('Retry with another CLI');
    await page.getByTestId('brainstorm-submit').click();
    await expectRequest(page, 4, secondCli, 'Retry with another CLI');
    await respond(page, { isComplete: true, generatedPrompt: GENERATED_PROMPT });
    await page.getByTestId('brainstorm-start').waitFor();
    assert.equal(await selector.count(), 1, 'completion keeps the same selector');
    assert.equal(await selector.inputValue(), secondCli);

    // Going back restores the question without resetting the chosen CLI.
    await selector.selectOption(thirdCli);
    await page.getByRole('button', { name: 'Back', exact: true }).click();
    assert.equal(await selector.inputValue(), thirdCli);
    assert.equal(await page.getByTestId('brainstorm-input').inputValue(), 'Retry with another CLI');
    await page.getByTestId('brainstorm-input').press('Enter');
    await expectRequest(page, 5, thirdCli, 'Retry with another CLI');
    await respond(page, { isComplete: true, generatedPrompt: GENERATED_PROMPT });
    await page.getByTestId('brainstorm-start').click();
    await page.getByTestId('task-start').waitFor();
    assert.deepEqual(await callsFor(page, 'complete_ai_brainstorm'), [{
      command: 'complete_ai_brainstorm',
      args: { projectId: 'project-1', generatedPrompt: GENERATED_PROMPT, cli: thirdCli, maxIterations: 7 }
    }], 'the completed task uses the current shared CLI selection');
    assert.equal((await callsFor(page, 'generate_project_title_cmd')).length, 1);
    assert.equal((await callsFor(page, 'save_config')).length, 0, 'per-question choice must not persist a global default');

    // Opening another project also checks the live settings store, not only IPC.
    await page.getByTestId('new-project').click();
    await selector.waitFor();
    assert.equal(await selector.inputValue(), defaultCli, 'the next project still inherits the original global default');
    assert.deepEqual(await page.evaluate(() => window.__brainstormTest.unexpected), []);
    assert.deepEqual(pageErrors, []);
    console.log(`PASS: default=${defaultCli}, first request=${firstCli}; text, single/multi-select, retry, back, completion`);
  } catch (error) {
    console.error('Browser diagnostics:', JSON.stringify({
      calls: await page.evaluate(() => window.__brainstormTest?.calls),
      pageErrors,
      browserErrors,
      body: await page.locator('body').innerText()
    }, null, 2));
    throw error;
  } finally {
    await context.close();
  }
}

async function main() {
  const port = Number(process.env.E2E_PORT || await availablePort());
  const url = `http://${HOST}:${port}`;
  let output = '';
  const child = spawn(process.execPath, [
    fileURLToPath(new URL('../../node_modules/vite/bin/vite.js', import.meta.url)),
    '--host', HOST, '--port', String(port), '--strictPort'
  ], {
    cwd: ROOT, env: { ...process.env, VITE_E2E: '0' }, stdio: ['ignore', 'pipe', 'pipe']
  });
  child.stdout.on('data', data => { output += data; });
  child.stderr.on('data', data => { output += data; });
  let browser;
  try {
    await waitForServer(url, child, () => output);
    browser = await chromium.launch({ headless: true });
    for (const cli of CLIS) await runScenario(browser, url, cli);
    await runScenario(browser, url, 'claude', 'opencode');
  } finally {
    await browser?.close();
    child.kill('SIGTERM');
  }
}

main().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
