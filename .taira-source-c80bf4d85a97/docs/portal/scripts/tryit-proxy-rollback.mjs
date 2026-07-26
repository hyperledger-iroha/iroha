#!/usr/bin/env node

import {promises as fs} from 'node:fs';
import path from 'node:path';
import process from 'node:process';

const COMMANDS = new Set(['update', 'rollback']);

function usage() {
  console.error(
    'Usage:\n' +
      '  node scripts/tryit-proxy-rollback.mjs update --target <url> [--env <file>]\n' +
      '  node scripts/tryit-proxy-rollback.mjs rollback [--env <file>]\n' +
      '\n' +
      'Environment:\n' +
      '  TRYIT_PROXY_ENV         Override env file path (.env.tryit-proxy by default)\n' +
      '  TRYIT_PROXY_NEW_TARGET  Target URL to apply when using the update command',
  );
}

function parseArgs(argv) {
  const [command, ...rest] = argv;
  if (!COMMANDS.has(command)) {
    usage();
    process.exit(1);
  }

  const options = {command, target: '', envFile: ''};
  for (let i = 0; i < rest.length; i += 1) {
    const arg = rest[i];
    if (arg === '--target') {
      options.target = rest[i + 1] ?? '';
      i += 1;
    } else if (arg === '--env') {
      options.envFile = rest[i + 1] ?? '';
      i += 1;
    } else {
      console.error(`Unknown argument: ${arg}`);
      usage();
      process.exit(1);
    }
  }
  return options;
}

async function fileExists(filePath) {
  try {
    await fs.access(filePath);
    return true;
  } catch (error) {
    if (error.code === 'ENOENT') {
      return false;
    }
    throw error;
  }
}

async function loadEnvFile(envFile) {
  try {
    const content = await fs.readFile(envFile, 'utf8');
    return {content, map: parseEnv(content)};
  } catch (error) {
    if (error.code === 'ENOENT') {
      return {content: '', map: new Map()};
    }
    throw error;
  }
}

function parseEnv(content) {
  const map = new Map();
  for (const line of content.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (trimmed === '' || trimmed.startsWith('#')) {
      continue;
    }
    const eq = trimmed.indexOf('=');
    if (eq === -1) {
      continue;
    }
    const key = trimmed.slice(0, eq).trim();
    const value = trimmed.slice(eq + 1).trim();
    map.set(key, value);
  }
  return map;
}

function serialiseEnv(map) {
  const lines = [];
  for (const [key, value] of map.entries()) {
    lines.push(`${key}=${value}`);
  }
  return `${lines.join('\n')}\n`;
}

async function handleUpdate(envFile, target) {
  if (!target) {
    target = process.env.TRYIT_PROXY_NEW_TARGET ?? '';
  }
  if (!target) {
    console.error('Missing target URL. Pass --target or set TRYIT_PROXY_NEW_TARGET.');
    process.exit(1);
  }

  const {content, map} = await loadEnvFile(envFile);
  if (content !== '') {
    const backupFile = `${envFile}.bak`;
    await fs.writeFile(backupFile, content);
    console.log(`[tryit-proxy-manage] backed up previous config to ${backupFile}`);
  }

  map.set('TRYIT_PROXY_TARGET', target);
  const output = serialiseEnv(map);
  await fs.writeFile(envFile, output, 'utf8');
  console.log(`[tryit-proxy-manage] updated TRYIT_PROXY_TARGET=${target}`);
}

async function handleRollback(envFile) {
  const backupFile = `${envFile}.bak`;
  if (!(await fileExists(backupFile))) {
    console.error(`[tryit-proxy-manage] backup file not found: ${backupFile}`);
    process.exit(1);
  }
  const backup = await fs.readFile(backupFile, 'utf8');
  await fs.writeFile(envFile, backup, 'utf8');
  console.log(`[tryit-proxy-manage] restored configuration from ${backupFile}`);
}

async function main() {
  const {command, target, envFile: cliEnv} = parseArgs(process.argv.slice(2));
  const envFile =
    cliEnv ||
    process.env.TRYIT_PROXY_ENV ||
    path.resolve(process.cwd(), '.env.tryit-proxy');

  if (command === 'update') {
    await handleUpdate(envFile, target);
  } else {
    await handleRollback(envFile);
  }
}

main().catch((error) => {
  console.error('[tryit-proxy-manage] unexpected error:', error);
  process.exit(1);
});
