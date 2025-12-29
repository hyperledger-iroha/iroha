#!/usr/bin/env node

import {createHash} from 'node:crypto';
import {promises as fs} from 'node:fs';
import process from 'node:process';
import {join, resolve} from 'node:path';
import {pathToFileURL} from 'node:url';

const DEFAULT_BUILD_DIR = resolve(process.cwd(), 'build');
const REPORT_NAME = 'link-report.json';
const MANIFEST_NAME = 'checksums.sha256';

export function parseOptions(argv) {
  const options = {
    buildDir: DEFAULT_BUILD_DIR,
  };
  for (const arg of argv) {
    if (arg.startsWith('--build-dir=')) {
      options.buildDir = resolve(process.cwd(), arg.slice('--build-dir='.length));
    }
  }
  return options;
}

export function parseSitemap(xmlContent) {
  if (!xmlContent) {
    return [];
  }
  const matches = [...xmlContent.matchAll(/<loc>(.+?)<\/loc>/g)];
  return matches.map(([, loc]) => loc.trim()).filter(Boolean);
}

export function urlToPathname(urlString) {
  try {
    const parsed = new URL(urlString);
    return decodeURIComponent(parsed.pathname);
  } catch {
    return '';
  }
}

export function relativePathsFor(pathname) {
  let normalized = pathname || '/';
  if (!normalized.startsWith('/')) {
    normalized = `/${normalized}`;
  }
  const trimmed = normalized.replace(/^\/+/, '').replace(/\/+$/, '');
  const candidates = new Set();
  if (trimmed === '') {
    candidates.add('index.html');
    return [...candidates];
  }
  const hasExtension = /\.[a-z0-9]+$/i.test(trimmed);
  if (hasExtension) {
    candidates.add(trimmed);
  } else {
    candidates.add(`${trimmed}.html`);
  }
  candidates.add(`${trimmed}/index.html`);
  candidates.add(trimmed);
  return [...candidates];
}

async function fileExists(path) {
  try {
    await fs.access(path);
    return true;
  } catch {
    return false;
  }
}

async function loadReleaseInfo(buildDir) {
  try {
    const data = await fs.readFile(join(buildDir, 'release.json'), 'utf8');
    return JSON.parse(data);
  } catch {
    return null;
  }
}

function countManifestEntries(content) {
  if (!content) {
    return 0;
  }
  return content
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean).length;
}

export async function loadManifestMetadata(buildDir) {
  const manifestPath = join(buildDir, MANIFEST_NAME);
  try {
    const data = await fs.readFile(manifestPath);
    const text = data.toString('utf8');
    const hash = createHash('sha256').update(data).digest('hex');
    return {
      id: hash,
      path: MANIFEST_NAME,
      bytes: data.length,
      entries: countManifestEntries(text),
    };
  } catch (error) {
    if (error && (error.code === 'ENOENT' || error.code === 'ENOTDIR')) {
      return null;
    }
    throw error;
  }
}

async function main() {
  const options = parseOptions(process.argv.slice(2));
  const sitemapPath = join(options.buildDir, 'sitemap.xml');
  const sitemap = await fs.readFile(sitemapPath, 'utf8');
  const urls = parseSitemap(sitemap);
  const releaseInfo = await loadReleaseInfo(options.buildDir);
  const manifestInfo = await loadManifestMetadata(options.buildDir);
  const entries = [];

  for (const url of urls) {
    const pathname = urlToPathname(url);
    const candidates = relativePathsFor(pathname);
    let resolved = null;
    for (const candidate of candidates) {
      const absolute = join(options.buildDir, candidate);
      if (await fileExists(absolute)) {
        resolved = candidate;
        break;
      }
    }
    entries.push({
      url,
      pathname,
      resolved,
      ok: Boolean(resolved),
      tried: candidates,
    });
  }

  const missing = entries.filter((entry) => !entry.ok);
  const report = {
    generated_at: new Date().toISOString(),
    release: releaseInfo,
    manifest: manifestInfo,
    totals: {
      checked: entries.length,
      missing: missing.length,
    },
    entries,
  };
  const reportPath = join(options.buildDir, REPORT_NAME);
  await fs.writeFile(reportPath, JSON.stringify(report, null, 2) + '\n', 'utf8');

  if (missing.length > 0) {
    console.error('[check-links] missing pages detected:');
    for (const entry of missing) {
      console.error(`- ${entry.url} (looked for ${entry.tried.join(', ')})`);
    }
    process.exitCode = 1;
  } else {
    console.log('[check-links] all sitemap entries resolved locally');
  }
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  main().catch((error) => {
    console.error('[check-links] fatal error', error);
    process.exitCode = 1;
  });
}
