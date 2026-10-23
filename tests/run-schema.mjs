// Use the Windows Python launcher or the Unix Python 3 executable, without a shell.
import { spawnSync } from 'node:child_process';

const windows = process.platform === 'win32';
const result = spawnSync(windows ? 'py' : 'python3', [
  ...(windows ? ['-3'] : []), '-m', 'unittest', 'discover', '-s', 'tests', '-v',
], { stdio: 'inherit', cwd: new URL('..', import.meta.url) });
if (result.error) console.error(`Cannot start Python 3: ${result.error.message}`);
process.exit(result.status ?? 1);
