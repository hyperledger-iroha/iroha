import assert from 'node:assert/strict';
import test from 'node:test';

import portalConfig from '../../docusaurus.config.js';
import {
  extractReleaseMeta,
  extractSecurityMeta,
  parseArgs,
  validateSecurityMeta,
} from '../portal-probe.mjs';

const defaultSecurity = portalConfig.customFields.security ?? {
  csp: '',
  permissionsPolicy: '',
  referrerPolicy: '',
};

test('parseArgs falls back to defaults', () => {
  const parsed = parseArgs([]);
  assert.equal(parsed.baseUrl, process.env.PORTAL_BASE_URL ?? 'http://localhost:3000');
  assert.deepEqual(parsed.paths, ['/', '/norito/overview', '/norito/streaming', '/devportal/try-it', '/reference/torii-swagger', '/reference/torii-rapidoc']);
  assert.equal(parsed.checkSecurity, true);
  assert.deepEqual(parsed.expectedSecurity, {
    csp: process.env.PORTAL_EXPECT_CSP ?? defaultSecurity.csp,
    permissionsPolicy:
      process.env.PORTAL_EXPECT_PERMISSIONS ?? defaultSecurity.permissionsPolicy,
    referrerPolicy: process.env.PORTAL_EXPECT_REFERRER ?? defaultSecurity.referrerPolicy,
  });
});

test('parseArgs accepts overrides', () => {
  const parsed = parseArgs([
    '--base-url=https://docs.example',
    '--expect-release=v1',
    '--paths=/one,/two',
    '--skip-security-checks',
    '--expect-csp=default-src example',
    '--expect-permissions=payment=()',
    '--expect-referrer=origin',
  ]);
  assert.equal(parsed.baseUrl, 'https://docs.example');
  assert.equal(parsed.expectRelease, 'v1');
  assert.deepEqual(parsed.paths, ['/one', '/two']);
  assert.equal(parsed.checkSecurity, false);
  assert.deepEqual(parsed.expectedSecurity, {
    csp: 'default-src example',
    permissionsPolicy: 'payment=()',
    referrerPolicy: 'origin',
  });
});

test('extractReleaseMeta returns meta content', () => {
  const html =
    '<html><head><meta name="sora-release" content="release-abc" /></head><body></body></html>';
  assert.equal(extractReleaseMeta(html), 'release-abc');
  assert.equal(extractReleaseMeta('<html></html>'), '');
});

test('extractSecurityMeta returns expected policy values', () => {
  const html = `
    <meta http-equiv="Content-Security-Policy" content="default-src 'self'">
    <meta http-equiv="Permissions-Policy" content="camera=()">
    <meta name="referrer" content="no-referrer">
  `;
  const meta = extractSecurityMeta(html);
  assert.equal(meta.csp, "default-src 'self'");
  assert.equal(meta.permissionsPolicy, 'camera=()');
  assert.equal(meta.referrerPolicy, 'no-referrer');
});

test('validateSecurityMeta reports missing and mismatched values', () => {
  const issues = validateSecurityMeta(
    {csp: 'default-src self', permissionsPolicy: '', referrerPolicy: 'origin'},
    {csp: 'default-src self', permissionsPolicy: 'payment=()', referrerPolicy: 'no-referrer'},
  );
  assert.deepEqual(issues, [
    'missing Permissions-Policy',
    'Referrer-Policy mismatch (expected "no-referrer", saw "origin")',
  ]);
});
