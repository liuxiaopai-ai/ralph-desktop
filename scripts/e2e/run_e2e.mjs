import { spawn } from 'node:child_process';
import net from 'node:net';
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { chromium } from 'playwright';

const PORT = Number(process.env.E2E_PORT || 1420);
const HOST = process.env.E2E_HOST || 'localhost';
const ARTIFACTS_DIR = path.resolve(process.cwd(), 'artifacts', 'e2e');
const CLI_ARG = process.argv.includes('--cli')
  ? process.argv[process.argv.indexOf('--cli') + 1]
  : null;

const CLIS = CLI_ARG ? [CLI_ARG] : ['codex', 'claude'];

function resolveProjectPath(cli) {
  const basePath = process.env.E2E_PROJECT_PATH;
  if (basePath) {
    return CLIS.length > 1 ? path.join(basePath, cli) : basePath;
  }
  return path.join('/tmp', `ralph-e2e-${Date.now()}-${cli}`);
}

function waitForPort(port, timeoutMs = 60000) {
  const started = Date.now();
  return new Promise((resolve, reject) => {
    const tryConnect = () => {
      const socket = net.createConnection({ port, host: HOST }, () => {
        socket.end();
        resolve();
      });
      socket.on('error', () => {
        socket.destroy();
        if (Date.now() - started > timeoutMs) {
          reject(new Error(`Timed out waiting for port ${port}`));
        } else {
          setTimeout(tryConnect, 500);
        }
      });
    };
    tryConnect();
  });
}

function startDevServer(cli, projectPath) {
  const env = {
    ...process.env,
    VITE_E2E: '1',
    VITE_E2E_CLI: cli,
    VITE_E2E_PORT: String(PORT),
    VITE_E2E_PROJECT_PATH: projectPath
  };

  const child = spawn('pnpm', ['dev', '--', '--host', HOST, '--port', String(PORT)], {
    env,
    stdio: 'inherit'
  });

  return { child, ready: waitForPort(PORT, 90000) };
}

async function runScenario(cli, mode) {
  const projectPath = resolveProjectPath(cli);
  const { child, ready } = startDevServer(cli, projectPath);
  try {
    await ready;

    const browser = await chromium.launch({ headless: true });
    const page = await browser.newPage();
    page.on('dialog', (dialog) => dialog.accept());

    await page.goto(`http://${HOST}:${PORT}`, { waitUntil: 'networkidle' });

    await page.getByTestId('new-project').click();

    await page.getByTestId('brainstorm-input').waitFor({ timeout: 20000 });
    const prompt = process.env.E2E_PROMPT || (cli === 'codex'
      ? '写一个与众不同的贪吃蛇'
      : '写一个与众不同的俄罗斯方块');
    await page.getByTestId('brainstorm-input').fill(prompt);
    await page.getByTestId('brainstorm-submit').click();

    await page.getByTestId('brainstorm-start').waitFor({ timeout: 20000 });
    await page.getByTestId('brainstorm-start').click();

    await page.getByTestId('task-start').waitFor({ timeout: 20000 });
    await page.getByTestId('task-start').click();

    await page.getByTestId('task-pause').waitFor({ timeout: 20000 });
    await page.getByTestId('task-pause').click();
    await page.getByTestId('task-resume').waitFor({ timeout: 20000 });
    await page.getByTestId('task-resume').click();

    await page.getByTestId('log-line').first().waitFor({ timeout: 20000 });

    if (mode === 'stop') {
      await page.getByTestId('task-stop').click();
      await page.waitForFunction(() => {
        const el = document.querySelector('[data-testid="task-status"]');
        return el && el.dataset.status === 'cancelled';
      }, null, { timeout: 20000 });
    } else {
      await page.waitForFunction(() => {
        const el = document.querySelector('[data-testid="task-status"]');
        return el && el.dataset.status === 'done';
      }, null, { timeout: 120000 });
    }

    if (mode === 'complete') {
      fs.mkdirSync(projectPath, { recursive: true });
      const html = `<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>E2E Output</title>
  <style>body{font-family:system-ui;background:#111;color:#eee;display:flex;align-items:center;justify-content:center;height:100vh;margin:0}canvas{border:1px solid #444}</style>
</head>
<body>
  <canvas id="game" width="640" height="480"></canvas>
  <script src="game.js"></script>
</body>
</html>
`;
      const js = `const canvas = document.getElementById('game');
const ctx = canvas.getContext('2d');
ctx.fillStyle = '#4cc2ff';
ctx.fillRect(40, 40, 80, 80);
ctx.fillStyle = '#fff';
ctx.fillText('E2E mock output', 40, 140);
`;
      fs.writeFileSync(path.join(projectPath, 'index.html'), html, 'utf8');
      fs.writeFileSync(path.join(projectPath, 'game.js'), js, 'utf8');
      console.log(`E2E output written to ${projectPath}`);
    }

    const artifactDir = path.join(ARTIFACTS_DIR, cli);
    fs.mkdirSync(artifactDir, { recursive: true });
    await page.screenshot({ path: path.join(artifactDir, `final-${mode}.png`), fullPage: true });

    await browser.close();
  } finally {
    child.kill('SIGTERM');
  }
}

async function main() {
  fs.mkdirSync(ARTIFACTS_DIR, { recursive: true });

  const singleMode = process.env.E2E_MODE;
  for (const [index, cli] of CLIS.entries()) {
    const mode = CLIS.length === 1 ? (singleMode || 'complete') : (index === 0 ? 'stop' : 'complete');
    await runScenario(cli, mode);
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
