import { spawn } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import { chromium } from '@playwright/test';

const output = process.env.EVAL_CHART_DIR ?? 'chart-artifacts';
const port = Number(process.env.EVAL_VIEWER_PORT ?? 4173);
const server = spawn('npm', ['run', 'preview', '--', '--host', '127.0.0.1', '--port', String(port)], {
  stdio: 'ignore'
});

try {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 1600, height: 1000 }, deviceScaleFactor: 2 });
  for (let attempt = 0; attempt < 40; attempt += 1) {
    try { const response = await page.goto(`http://127.0.0.1:${port}/`); if (response?.ok()) break; }
    catch { if (attempt === 39) throw new Error('viewer did not start'); }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  await page.getByText('comparable results').waitFor();
  await mkdir(output, { recursive: true });
  const plot = page.locator('#pareto-plot');
  await writeFile(`${output}/pareto.svg`, `${await plot.evaluate((node) => node.outerHTML)}\n`);
  await plot.screenshot({ path: `${output}/pareto.png` });
  await writeFile(`${output}/source-url.txt`, `${page.url()}\n`);
  await browser.close();
} finally {
  server.kill('SIGTERM');
}
