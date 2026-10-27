import { readdir, readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const root = fileURLToPath(new URL('../', import.meta.url));
const excluded = new Set(['node_modules', 'target', '.git', '.tools', 'dist', 'var']);
let failures = 0;

async function inspect(directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (excluded.has(entry.name)) continue;
    const filename = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      await inspect(filename);
    } else if (/\.(rs|tsx?|mjs)$/.test(entry.name)) {
      const contents = await readFile(filename, 'utf8');
      const count = contents.split('\n').filter(line => {
        const text = line.trim();
        return text.length > 0 && !text.startsWith('//') && !text.startsWith('*') && !text.startsWith('/*');
      }).length;
      if (count > 250) {
        console.error(`FAIL ${path.relative(root, filename)}: ${count} code lines (limit 250)`);
        failures += 1;
      } else if (count > 200) {
        console.info(`REVIEW ${path.relative(root, filename)}: ${count} code lines`);
      }
    }
  }
}

await inspect(root);
if (failures > 0) process.exitCode = 1;
else console.info('Source-size review passed: all source modules <=250 code lines.');
