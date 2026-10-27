import { readdir, readFile, readlink } from 'node:fs/promises';

const root = new URL('../', import.meta.url).pathname.replace(/\/$/, '');

export async function nativeInstances() {
  const instances = [];
  for (const entry of await readdir('/proc')) {
    if (!/^\d+$/.test(entry)) continue;
    try {
      const executable = await readlink(`/proc/${entry}/exe`);
      if (!executable.startsWith(`${root}/`) || !executable.replace(/ \(deleted\)$/, '').endsWith('/live-production')) continue;
      const stat = await readFile(`/proc/${entry}/stat`, 'utf8');
      const status = await readFile(`/proc/${entry}/status`, 'utf8');
      const fields = stat.slice(stat.lastIndexOf(') ') + 2).trim().split(/\s+/);
      instances.push({ pid: Number(entry), parentPid: Number(fields[1]), generation: fields[19],
        threads: Number(status.match(/^Threads:\s+(\d+)/m)?.[1] ?? 0), deleted: executable.endsWith(' (deleted)') });
    } catch (error) {
      if (!(error instanceof Error) || !('code' in error) || !['ENOENT', 'ESRCH', 'EACCES'].includes(error.code)) throw error;
    }
  }
  return instances.sort((a, b) => a.pid - b.pid);
}
