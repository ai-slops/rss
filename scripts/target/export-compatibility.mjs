#!/usr/bin/env node

import { randomUUID } from 'node:crypto';
import { mkdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { buildPortablePackage } from './support/export-package.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');

async function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.help) {
    process.stdout.write(`${usage()}\n`);
    return 0;
  }
  if (!options.input) throw new Error('Missing --input editor package JSON');
  const output = options.output ?? path.join(root, 'var', 'target', `export-package-${randomUUID()}`);
  await mkdir(path.dirname(output), { recursive: true, mode: 0o700 });
  const parsed = JSON.parse(await readFile(options.input, 'utf8'));
  const input = parsed?.pluginResult ?? parsed;
  const result = await buildPortablePackage(input, output, {
    supervisor: options.supervisor ?? process.env.LIVE_PRODUCTION_SUPERVISOR
      ?? path.join(root, 'target', 'debug', 'live-production-supervisor'),
    ffprobe: options.ffprobe ?? process.env.FFPROBE ?? 'ffprobe',
    ffmpeg: options.ffmpeg ?? process.env.FFMPEG ?? 'ffmpeg',
  });
  const outcome = {
    outputDirectory: output,
    structural: result.report.statuses.structural.status,
    decoder: result.report.statuses.decoder.status,
    applicationImport: result.report.statuses.applicationImport.status,
    compatibilityReport: path.join(output, 'metadata', 'compatibility-report.json'),
  };
  process.stdout.write(`${JSON.stringify(outcome, null, 2)}\n`);
  return outcome.structural === 'passed' && outcome.decoder === 'passed' ? 0 : 1;
}

function parseArguments(arguments_) {
  const options = {};
  for (let index = 0; index < arguments_.length; index += 1) {
    const value = arguments_[index];
    if (value === '--help' || value === '-h') options.help = true;
    else if (value === '--input') options.input = requiredValue(arguments_, ++index, value);
    else if (value === '--output') options.output = requiredValue(arguments_, ++index, value);
    else if (value === '--supervisor') options.supervisor = requiredValue(arguments_, ++index, value);
    else if (value === '--ffprobe') options.ffprobe = requiredValue(arguments_, ++index, value);
    else if (value === '--ffmpeg') options.ffmpeg = requiredValue(arguments_, ++index, value);
    else throw new Error(`Unknown argument: ${value}`);
  }
  return options;
}

function requiredValue(arguments_, index, flag) {
  const value = arguments_[index];
  if (!value || value.startsWith('--')) throw new Error(`Missing value for ${flag}`);
  return value;
}

function usage() {
  return 'Usage: node scripts/target/export-compatibility.mjs --input <editor-package.json> [--output <private-directory>] [--supervisor <path>] [--ffprobe <path>] [--ffmpeg <path>]';
}

main().then(
  code => { process.exitCode = code; },
  error => {
    process.stderr.write(`export compatibility: ${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  },
);
