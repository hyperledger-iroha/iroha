#!/usr/bin/env node
/**
 * Ensures the SDK ledger recipe documentation stays in sync with canonical sample files.
 *
 * Usage:
 *   node docs/portal/scripts/check-sdk-recipes.mjs
 *       - verifies docs match the samples (default, fails on drift)
 *   node docs/portal/scripts/check-sdk-recipes.mjs --write-docs
 *       - rewrites the markdown code blocks using the sample contents
 *   node docs/portal/scripts/check-sdk-recipes.mjs --write-samples
 *       - regenerates the sample files from the current markdown blocks
 */

import fs from 'node:fs';
import path from 'node:path';
import url from 'node:url';

const repoRoot = path.resolve(
  path.dirname(url.fileURLToPath(import.meta.url)),
  '..',
  '..',
  '..',
);
const recipes = [
  {
    doc: 'docs/portal/docs/sdks/recipes/javascript-ledger-flow.md',
    sample: 'docs/portal/static/sdk-recipes/javascript/ledger-flow.mjs',
    lang: 'ts',
    title: 'ledger-flow.mjs',
  },
  {
    doc: 'docs/portal/docs/sdks/recipes/python-ledger-flow.md',
    sample: 'docs/portal/static/sdk-recipes/python/ledger_flow.py',
    lang: 'python',
    title: 'ledger_flow.py',
  },
  {
    doc: 'docs/portal/docs/sdks/recipes/rust-ledger-flow.md',
    sample: 'docs/portal/static/sdk-recipes/rust/src/main.rs',
    lang: 'rust',
    title: 'src/main.rs',
  },
  {
    doc: 'docs/portal/docs/sdks/recipes/java-ledger-flow.md',
    sample: 'docs/portal/static/sdk-recipes/java/src/main/java/ledger/LedgerFlow.java',
    lang: 'java',
    title: 'src/main/java/ledger/LedgerFlow.java',
  },
  {
    doc: 'docs/portal/docs/sdks/recipes/swift-ledger-flow.md',
    sample: 'docs/portal/static/sdk-recipes/swift/Sources/LedgerFlow/main.swift',
    lang: 'swift',
    title: 'Sources/LedgerFlow/main.swift',
  },
];

const args = new Set(process.argv.slice(2));
const mode = args.has('--write-docs')
  ? 'write-docs'
  : args.has('--write-samples')
    ? 'write-samples'
    : 'check';

function escapeRegex(value) {
  return value.replace(/[-/\\^$*+?.()|[\]{}]/g, '\\$&');
}

function normalize(content) {
  return `${content.replace(/\r\n/g, '\n').replace(/\s+$/, '')}\n`;
}

function buildRegex({lang, title}) {
  const titlePart = title ? `[^\\n]*title="${escapeRegex(title)}"[^\\n]*` : '';
  return new RegExp(`(\\\`\\\`\\\`${lang}${titlePart}\\n)([\\s\\S]*?)(\\\`\\\`\\\`)`, 'm');
}

function extractSnippet(docContent, entry) {
  const regex = buildRegex(entry);
  const match = regex.exec(docContent);
  if (!match) {
    throw new Error(`failed to locate ${entry.lang} block (${entry.title}) in ${entry.doc}`);
  }
  const [full, opening, body, closing] = match;
  const start = match.index;
  const end = start + full.length;
  return {
    snippet: body,
    opening,
    closing,
    start,
    end,
  };
}

function ensureDir(filePath) {
  fs.mkdirSync(path.dirname(filePath), {recursive: true});
}

let driftDetected = false;

for (const entry of recipes) {
  const docPath = path.join(repoRoot, entry.doc);
  const samplePath = path.join(repoRoot, entry.sample);
  const docContent = fs.readFileSync(docPath, 'utf8');
  const {snippet, opening, closing, start, end} = extractSnippet(docContent, entry);
  if (mode === 'write-samples') {
    ensureDir(samplePath);
    fs.writeFileSync(samplePath, normalize(snippet), 'utf8');
    console.log(`[sdk-recipes] wrote ${entry.sample} from ${entry.doc}`);
    continue;
  }

  if (!fs.existsSync(samplePath)) {
    throw new Error(`missing sample ${entry.sample}; run with --write-samples to bootstrap`);
  }

  const sampleContent = normalize(fs.readFileSync(samplePath, 'utf8'));
  const normalizedSnippet = normalize(snippet);

  if (mode === 'write-docs') {
    if (normalizedSnippet === sampleContent) {
      continue;
    }
    const updated = `${docContent.slice(0, start)}${opening}${sampleContent}${closing}${docContent.slice(end)}`;
    fs.writeFileSync(docPath, updated, 'utf8');
    console.log(`[sdk-recipes] updated ${entry.doc} using ${entry.sample}`);
    continue;
  }

  if (normalizedSnippet !== sampleContent) {
    driftDetected = true;
    console.error(`[sdk-recipes] ${entry.doc} differs from ${entry.sample}`);
  }
}

if (mode === 'check' && driftDetected) {
  console.error('[sdk-recipes] run `node docs/portal/scripts/check-sdk-recipes.mjs --write-docs` to sync markdown');
  process.exit(1);
}
