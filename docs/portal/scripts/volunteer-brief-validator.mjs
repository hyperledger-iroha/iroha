#!/usr/bin/env node

import {spawn} from 'node:child_process';
import {mkdir, readdir, readFile, writeFile} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

import {buildPortalSummary} from './volunteer-brief-validator-lib.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..', '..', '..');
const portalRoot = path.resolve(__dirname, '..');
const examplesDir = path.resolve(repoRoot, 'docs', 'examples', 'ministry');
const cliReportPath = path.resolve(portalRoot, '.docusaurus', 'volunteer-lint-cli.json');
const outputPath = path.resolve(portalRoot, 'src', 'data', 'volunteerLintStatus.json');

async function main() {
  const inputs = await collectVolunteerInputs();
  if (inputs.length === 0) {
    const summary = buildPortalSummary(
      {
        entries: [],
        total_entries: 0,
        entries_with_errors: 0,
        total_errors: 0,
        total_warnings: 0,
        inputs: []
      },
      {repoRoot}
    );
    await writeJson(outputPath, summary);
    console.warn(
      '[volunteer-brief-validator] no volunteer briefs were found; wrote empty summary'
    );
    return;
  }

  await runCargoValidator(inputs);
  const cliRaw = await readFile(cliReportPath, 'utf8');
  const cliReport = JSON.parse(cliRaw);
  const summary = buildPortalSummary(cliReport, {repoRoot});
  await writeJson(outputPath, summary);
  console.log(
    `[volunteer-brief-validator] validated ${inputs.length} file(s); wrote ${path.relative(
      repoRoot,
      outputPath
    )}`
  );
}

async function collectVolunteerInputs() {
  try {
    const entries = await readdir(examplesDir, {withFileTypes: true});
    return entries
      .filter((entry) => entry.isFile() && /^volunteer_.*\.json$/i.test(entry.name))
      .map((entry) => path.join(examplesDir, entry.name))
      .sort();
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      return [];
    }
    throw error;
  }
}

async function runCargoValidator(inputs) {
  await mkdir(path.dirname(cliReportPath), {recursive: true});
  const args = [
    'xtask',
    'ministry-transparency',
    'volunteer-validate',
    '--json-output',
    cliReportPath,
    ...inputs.flatMap((input) => ['--input', input])
  ];
  await new Promise((resolve, reject) => {
    const child = spawn('cargo', args, {
      cwd: repoRoot,
      stdio: 'inherit',
      env: process.env
    });
    child.on('error', (err) => {
      reject(err);
    });
    child.on('exit', (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(
          new Error(`[volunteer-brief-validator] cargo exited with status ${code ?? 'unknown'}`)
        );
      }
    });
  });
}

async function writeJson(filePath, data) {
  await mkdir(path.dirname(filePath), {recursive: true});
  const text = `${JSON.stringify(data, null, 2)}\n`;
  await writeFile(filePath, text, 'utf8');
}

if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(__filename)) {
  main().catch((error) => {
    console.error('[volunteer-brief-validator] failed:', error);
    process.exitCode = 1;
  });
}
