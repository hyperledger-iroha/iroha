#!/usr/bin/env node

import {mkdir, readdir, readFile, rm, stat, writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import path from 'node:path';

import {SNIPPET_MARKER, SNIPPETS} from './norito-snippets-config.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const repoRoot = path.resolve(__dirname, '..', '..', '..');
const portalRoot = path.resolve(__dirname, '..');
const outputDocsDir = path.resolve(portalRoot, 'docs', 'norito', 'examples');
const staticDir = path.resolve(portalRoot, 'static', 'norito-snippets');
const manifestPath = path.resolve(portalRoot, '.docusaurus', 'norito-snippets-manifest.json');
const marker = SNIPPET_MARKER;
const MANIFEST_VERSION = 1;
const TEMPLATE_REVISION = 3;

function formatDoc(snippet, code) {
  const {slug, title, description, source, ledgerWalkthrough, sdkGuides} = snippet;
  const downloadPath = `/norito-snippets/${slug}.ko`;
  const relSource = source;
  const sections = [
    formatLedgerSection(ledgerWalkthrough),
    formatSdkGuideSection(sdkGuides)
  ].filter(Boolean);
  const sectionsBlock = sections.length > 0 ? `\n\n${sections.join('\n\n')}\n` : '\n\n';
  return `${marker}\n\n---\nslug: /norito/examples/${slug}\ntitle: ${title}\ndescription: ${description}\nsource: ${relSource}\n---\n\n${description}${sectionsBlock}\n[Download the Kotodama source](${downloadPath})\n\n\`\`\`text\n${code.trim()}\n\`\`\`\n`;
}

function formatIndex(snippets) {
  const items = snippets
    .map(({slug, title, description}) => `- **[${title}](./${slug})** — ${description}`)
    .join('\n');
  return `${marker}\n\n---\nslug: /norito/examples\ntitle: Norito examples\ndescription: Curated Kotodama snippets with ledger walkthroughs.\n---\n\nThese examples mirror the SDK quickstarts and ledger walkthroughs. Each snippet bundles a ledger checklist and links back to the Rust, Python, and JavaScript guides so you can replay the same scenario end to end.\n\n${items}\n`;
}

function formatLedgerSection(steps) {
  if (!Array.isArray(steps) || steps.length === 0) {
    return '';
  }
  const content = steps.map((step) => `- ${step}`).join('\n');
  return `## Ledger walkthrough\n\n${content}`;
}

function formatSdkGuideSection(guides) {
  if (!Array.isArray(guides) || guides.length === 0) {
    return '';
  }
  const content = guides
    .map(({label, permalink}) => {
      if (!label || !permalink) {
        return null;
      }
      return `- [${label}](${permalink})`;
    })
    .filter(Boolean)
    .join('\n');
  if (!content) {
    return '';
  }
  return `## Related SDK guides\n\n${content}`;
}

async function main() {
  await mkdir(outputDocsDir, {recursive: true});
  await mkdir(staticDir, {recursive: true});

  const descriptors = await buildSnippetDescriptors(SNIPPETS);
  const manifestEntries = createManifestEntries(descriptors);
  const previousManifest = await readManifest(manifestPath);
  const outputsReady = await outputsExist(SNIPPETS);

  const needsGeneration =
    manifestNeedsUpdate(previousManifest, manifestEntries) || !outputsReady;

  if (!needsGeneration) {
    console.log('[sync-norito-snippets] inputs unchanged; skipping generation');
    return;
  }

  const staleDocs = await collectGeneratedDocs(outputDocsDir);
  const staleStatic = await collectGeneratedStatic(staticDir);

  const generated = [];
  let updatedDocs = 0;
  let updatedStatic = 0;

  for (const {snippet, absoluteSource} of descriptors) {
    const code = await readFile(absoluteSource, 'utf8');
    const docPath = path.join(outputDocsDir, `${snippet.slug}.md`);
    const staticPath = path.join(staticDir, `${snippet.slug}.ko`);

    if (await writeIfChanged(docPath, formatDoc(snippet, code))) {
      updatedDocs += 1;
    }
    if (await writeIfChanged(staticPath, code)) {
      updatedStatic += 1;
    }

    staleDocs.delete(docPath);
    staleStatic.delete(staticPath);
    generated.push(snippet);
  }

  const indexPath = path.join(outputDocsDir, 'index.md');
  if (await writeIfChanged(indexPath, formatIndex(generated))) {
    updatedDocs += 1;
  }
  staleDocs.delete(indexPath);

  await removeStaleFiles(staleDocs);
  await removeStaleFiles(staleStatic);
  await writeManifest(manifestPath, manifestEntries);

  console.log(
    `[sync-norito-snippets] processed ${generated.length} snippets (${updatedDocs} docs updated, ${updatedStatic} snippets updated)`
  );
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : null;
if (invokedPath === __filename) {
  main().catch((error) => {
    console.error('[sync-norito-snippets] failed:', error);
    process.exit(1);
  });
}

async function collectGeneratedDocs(dir) {
  const entries = await readdir(dir, {withFileTypes: true});
  const generated = new Set();
  for (const entry of entries) {
    if (!entry.isFile()) continue;
    const filePath = path.join(dir, entry.name);
    const contents = await readFile(filePath, 'utf8');
    if (contents.includes(marker)) {
      generated.add(filePath);
    }
  }
  return generated;
}

async function collectGeneratedStatic(dir) {
  const entries = await readdir(dir, {withFileTypes: true});
  const generated = new Set();
  for (const entry of entries) {
    if (!entry.isFile() || path.extname(entry.name) !== '.ko') {
      continue;
    }
    generated.add(path.join(dir, entry.name));
  }
  return generated;
}

async function writeIfChanged(filePath, contents) {
  await mkdir(path.dirname(filePath), {recursive: true});
  const current = await readFileSafe(filePath);
  if (current === contents) {
    return false;
  }
  await writeFile(filePath, contents, 'utf8');
  return true;
}

async function readFileSafe(filePath) {
  try {
    const stats = await stat(filePath);
    if (!stats.isFile()) {
      return null;
    }
    return await readFile(filePath, 'utf8');
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      return null;
    }
    throw error;
  }
}

async function removeStaleFiles(paths) {
  for (const filePath of paths) {
    await rm(filePath);
  }
}

async function buildSnippetDescriptors(snippets) {
  const descriptors = [];
  for (const snippet of snippets) {
    const absoluteSource = path.join(repoRoot, snippet.source);
    const stats = await stat(absoluteSource);
    descriptors.push({snippet, absoluteSource, stats});
  }
  return descriptors;
}

function createManifestEntries(descriptors) {
  return descriptors.map(({snippet, stats}) => ({
    slug: snippet.slug,
    source: snippet.source,
    title: snippet.title,
    description: snippet.description,
    size: stats.size,
    mtimeMs: stats.mtimeMs,
    templateRevision: TEMPLATE_REVISION
  }));
}

export function manifestNeedsUpdate(previousManifest, nextEntries) {
  if (!previousManifest || previousManifest.version !== MANIFEST_VERSION) {
    return true;
  }
  if (!Array.isArray(previousManifest.entries)) {
    return true;
  }
  if (previousManifest.entries.length !== nextEntries.length) {
    return true;
  }

  const previousBySlug = new Map(
    previousManifest.entries.map((entry) => [entry.slug, entry])
  );

  for (const entry of nextEntries) {
    const previousEntry = previousBySlug.get(entry.slug);
    if (!previousEntry) {
      return true;
    }
    if (!manifestEntriesEqual(previousEntry, entry)) {
      return true;
    }
  }
  return false;
}

function manifestEntriesEqual(previousEntry, nextEntry) {
  return (
    previousEntry.source === nextEntry.source &&
    previousEntry.title === nextEntry.title &&
    previousEntry.description === nextEntry.description &&
    previousEntry.size === nextEntry.size &&
    previousEntry.mtimeMs === nextEntry.mtimeMs &&
    previousEntry.templateRevision === nextEntry.templateRevision
  );
}

async function outputsExist(snippets) {
  const indexPath = path.join(outputDocsDir, 'index.md');
  if (!(await fileExists(indexPath))) {
    return false;
  }

  for (const snippet of snippets) {
    const docPath = path.join(outputDocsDir, `${snippet.slug}.md`);
    const staticPath = path.join(staticDir, `${snippet.slug}.ko`);
    if (!(await fileExists(docPath)) || !(await fileExists(staticPath))) {
      return false;
    }
  }
  return true;
}

async function fileExists(filePath) {
  try {
    const stats = await stat(filePath);
    return stats.isFile();
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      return false;
    }
    throw error;
  }
}

async function readManifest(filePath) {
  try {
    const raw = await readFile(filePath, 'utf8');
    return JSON.parse(raw);
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      return null;
    }
    throw error;
  }
}

async function writeManifest(filePath, entries) {
  const payload = JSON.stringify(
    {
      version: MANIFEST_VERSION,
      generatedAt: new Date().toISOString(),
      entries
    },
    null,
    2
  );
  await mkdir(path.dirname(filePath), {recursive: true});
  await writeFile(filePath, payload, 'utf8');
}

export {MANIFEST_VERSION, TEMPLATE_REVISION, formatLedgerSection, formatSdkGuideSection};
