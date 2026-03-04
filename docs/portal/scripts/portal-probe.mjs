#!/usr/bin/env node

import {performance} from 'node:perf_hooks';
import process from 'node:process';
import {pathToFileURL} from 'node:url';

import portalConfig from '../docusaurus.config.js';

const DEFAULT_PATHS = [
  '/',
  '/norito/overview',
  '/norito/streaming',
  '/devportal/try-it',
  '/reference/torii-swagger',
  '/reference/torii-rapidoc',
];

const DEFAULT_SECURITY_EXPECTATIONS = (() => {
  const security = portalConfig?.customFields?.security ?? {};
  return {
    csp: security.csp ?? '',
    permissionsPolicy: security.permissionsPolicy ?? '',
    referrerPolicy: security.referrerPolicy ?? '',
  };
})();

export function parseArgs(argv) {
  const options = {
    baseUrl: process.env.PORTAL_BASE_URL ?? 'http://localhost:3000',
    expectRelease: process.env.DOCS_RELEASE_TAG ?? '',
    paths: [...DEFAULT_PATHS],
    checkSecurity: (process.env.PORTAL_CHECK_SECURITY ?? '1') !== '0',
    expectedSecurity: {
      csp: process.env.PORTAL_EXPECT_CSP ?? DEFAULT_SECURITY_EXPECTATIONS.csp,
      permissionsPolicy:
        process.env.PORTAL_EXPECT_PERMISSIONS ?? DEFAULT_SECURITY_EXPECTATIONS.permissionsPolicy,
      referrerPolicy:
        process.env.PORTAL_EXPECT_REFERRER ?? DEFAULT_SECURITY_EXPECTATIONS.referrerPolicy,
    },
  };

  for (const arg of argv) {
    if (arg.startsWith('--base-url=')) {
      options.baseUrl = arg.slice('--base-url='.length);
    } else if (arg.startsWith('--expect-release=')) {
      options.expectRelease = arg.slice('--expect-release='.length);
    } else if (arg.startsWith('--paths=')) {
      options.paths = arg
        .slice('--paths='.length)
        .split(',')
        .map((value) => value.trim())
        .filter(Boolean);
    } else if (arg === '--skip-security-checks') {
      options.checkSecurity = false;
    } else if (arg.startsWith('--expect-csp=')) {
      options.expectedSecurity.csp = arg.slice('--expect-csp='.length);
    } else if (arg.startsWith('--expect-permissions=')) {
      options.expectedSecurity.permissionsPolicy = arg.slice('--expect-permissions='.length);
    } else if (arg.startsWith('--expect-referrer=')) {
      options.expectedSecurity.referrerPolicy = arg.slice('--expect-referrer='.length);
    }
  }

  return options;
}

export function extractReleaseMeta(html) {
  return extractMetaContent(html, (attrs) => (attrs.name ?? '').toLowerCase() === 'sora-release');
}

export async function probePath(baseUrl, path) {
  const target = new URL(path, baseUrl);
  const start = performance.now();
  const response = await fetch(target, {redirect: 'manual'});
  const durationMs = performance.now() - start;
  const body = await response.text();
  const security = extractSecurityMeta(body);
  return {
    url: target.toString(),
    status: response.status,
    ok: response.ok,
    release: extractReleaseMeta(body),
    security,
    durationMs,
  };
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const failures = [];

  for (const path of options.paths) {
    try {
      const result = await probePath(options.baseUrl, path);
      const releaseMatches =
        !options.expectRelease || result.release === options.expectRelease;
      const securityIssues =
        options.checkSecurity && hasSecurityExpectations(options.expectedSecurity)
          ? validateSecurityMeta(result.security, options.expectedSecurity)
          : [];
      const ok = result.ok && releaseMatches && securityIssues.length === 0;
      const summary = `${result.url} [${result.status}] ${result.durationMs.toFixed(0)}ms`;
      if (ok) {
        const details = [`release=${result.release || 'n/a'}`];
        if (options.checkSecurity && hasSecurityExpectations(options.expectedSecurity)) {
          details.push('security=ok');
        }
        console.log('[portal-probe] OK', summary, details.join(' '));
      } else {
        const reasons = [];
        if (!result.ok) {
          reasons.push('HTTP error');
        }
        if (!releaseMatches) {
          reasons.push(
            `release mismatch (expected ${options.expectRelease}, saw ${result.release || 'n/a'})`,
          );
        }
        if (securityIssues.length > 0) {
          reasons.push(`security meta mismatch: ${securityIssues.join('; ')}`);
        }
        const reason = reasons.length > 0 ? reasons.join('; ') : 'unknown failure';
        console.error('[portal-probe] FAIL', summary, reason);
        failures.push({path, result, reason});
      }
    } catch (error) {
      console.error('[portal-probe] ERROR', path, error.message ?? error);
      failures.push({path, error});
    }
  }

  if (failures.length > 0) {
    console.error(`[portal-probe] ${failures.length} target(s) failed`);
    process.exitCode = 1;
  }
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  main().catch((error) => {
    console.error('[portal-probe] fatal error', error);
    process.exitCode = 1;
  });
}

export function extractSecurityMeta(html) {
  return {
    csp: extractMetaContent(
      html,
      (attrs) => (attrs['http-equiv'] ?? '').toLowerCase() === 'content-security-policy',
    ),
    permissionsPolicy: extractMetaContent(
      html,
      (attrs) => (attrs['http-equiv'] ?? '').toLowerCase() === 'permissions-policy',
    ),
    referrerPolicy: extractMetaContent(
      html,
      (attrs) => (attrs.name ?? '').toLowerCase() === 'referrer',
    ),
  };
}

function extractMetaContent(html, predicate) {
  if (!html) {
    return '';
  }
  const metaRegex = /<meta\b[^>]*>/gi;
  let match;
  while ((match = metaRegex.exec(html))) {
    const attrs = parseMetaAttributes(match[0]);
    if (predicate(attrs)) {
      return attrs.content ?? '';
    }
  }
  return '';
}

function parseMetaAttributes(tagLiteral) {
  const attrs = {};
  const attrRegex = /([a-zA-Z0-9:-]+)\s*=\s*(?:"([^"]*)"|'([^']*)')/g;
  let attrMatch;
  while ((attrMatch = attrRegex.exec(tagLiteral))) {
    const name = attrMatch[1].toLowerCase();
    const value = (attrMatch[2] ?? attrMatch[3] ?? '').trim();
    attrs[name] = value;
  }
  return attrs;
}

export function validateSecurityMeta(actual, expected) {
  const issues = [];
  const checks = [
    {key: 'csp', label: 'Content-Security-Policy'},
    {key: 'permissionsPolicy', label: 'Permissions-Policy'},
    {key: 'referrerPolicy', label: 'Referrer-Policy'},
  ];
  for (const {key, label} of checks) {
    const expectedValue = normaliseSecurityValue(expected[key]);
    if (!expectedValue) {
      continue;
    }
    const actualValue = normaliseSecurityValue(actual[key]);
    if (!actualValue) {
      issues.push(`missing ${label}`);
      continue;
    }
    if (actualValue !== expectedValue) {
      issues.push(`${label} mismatch (expected "${expectedValue}", saw "${actualValue}")`);
    }
  }
  return issues;
}

function normaliseSecurityValue(value) {
  return (value ?? '').replace(/\s+/g, ' ').trim();
}

function hasSecurityExpectations(expected) {
  return (
    normaliseSecurityValue(expected.csp) !== '' ||
    normaliseSecurityValue(expected.permissionsPolicy) !== '' ||
    normaliseSecurityValue(expected.referrerPolicy) !== ''
  );
}
