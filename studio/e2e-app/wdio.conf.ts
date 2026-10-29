// The built app, end to end (#73): WebdriverIO driving isolex studio
// through tauri-driver, which speaks WebDriver to the app's own webview
// (WebKitWebDriver on Linux, msedgedriver for WebView2 on Windows; macOS
// has no driver). The app is built first, unbundled, and opened on a copy
// of the compiler's sample library, given on its command line.
//
// npm run e2e:app, with tauri-driver on the PATH or in ~/.cargo/bin, and on
// Windows msedgedriver for the installed WebView2 on the PATH.

import { spawn, spawnSync, type ChildProcess } from 'node:child_process';
import { copyFileSync, mkdtempSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const studio = resolve(fileURLToPath(new URL('..', import.meta.url)));
const exe = process.platform === 'win32' ? '.exe' : '';
const application = join(studio, 'src-tauri', 'target', 'debug', `isolex-studio${exe}`);

// the library the app opens: a copy, which the test saves into. This file
// is read by the launcher, which starts the app, and again by the worker,
// which runs the test and inherits the launcher's environment: one copy,
// made once, or the test reads a copy the app never opened.
function copyOfSample(): string {
  const dir = mkdtempSync(join(tmpdir(), 'isolex-studio-e2e-'));
  copyFileSync(join(studio, '..', 'tests', 'fixtures', 'sample.ilx'), join(dir, 'sample.ilx'));
  return dir;
}
const library = (process.env.STUDIO_E2E_LIBRARY ??= copyOfSample());
// on Windows only, so that Linux tests the command line
if (process.platform === 'win32') process.env.ISOLEX_STUDIO_LIBRARY = library;

/** tauri-driver: on the PATH, or where cargo install puts it. */
function driver(): string {
  const found = spawnSync(process.platform === 'win32' ? 'where' : 'which', ['tauri-driver'], { encoding: 'utf8' });
  const first = found.status === 0 ? found.stdout.split(/\r?\n/)[0]?.trim() : '';
  return first || join(homedir(), '.cargo', 'bin', `tauri-driver${exe}`);
}

let tauriDriver: ChildProcess | undefined;

export const config: WebdriverIO.Config = {
  runner: 'local',
  hostname: '127.0.0.1',
  port: 4444,
  specs: ['./app.e2e.ts'],
  maxInstances: 1,
  capabilities: [
    {
      maxInstances: 1,
      // tauri-driver speaks classic WebDriver
      'wdio:enforceWebDriverClassic': true,
      // webviewOptions: msedgedriver drives WebView2 in the app, not
      // the app as the Edge browser (tauri-driver passes it on Windows only)
      // the library: on Linux on the command line; on Windows, where
      // msedgedriver gives args to WebView2 and not to the app, through
      // ISOLEX_STUDIO_LIBRARY, which reaches the app through tauri-driver
      // and msedgedriver
      'tauri:options': { application, args: process.platform === 'win32' ? [] : [library], webviewOptions: {} },
    } as WebdriverIO.Capabilities,
  ],
  logLevel: 'warn',
  reporters: ['spec'],
  framework: 'mocha',
  mochaOpts: { ui: 'bdd', timeout: 180_000 },

  // the app, built against the interface as the shell serves it: as a
  // production build, whatever NODE_ENV WebdriverIO sets (it sets "test",
  // which Vite takes for development, compiling in the browser's stand-in
  // core, which needs the WebAssembly module)
  onPrepare: () => {
    const env = { ...process.env, NODE_ENV: 'production' };
    const built = spawnSync('npm', ['run', 'tauri', '--', 'build', '--debug', '--no-bundle'], { cwd: studio, stdio: 'inherit', shell: true, env });
    if (built.status !== 0) throw new Error('the app did not build');
  },
  beforeSession: () => {
    tauriDriver = spawn(driver(), [], { stdio: [null, process.stdout, process.stderr] });
  },
  afterSession: () => {
    tauriDriver?.kill();
  },
};
