import assert from 'node:assert/strict';
import {execFile} from 'node:child_process';
import {mkdtemp, readFile, rm, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import test from 'node:test';
import {promisify} from 'node:util';

const execFileAsync = promisify(execFile);
const repoRoot = fileURLToPath(new URL('../../../../', import.meta.url));
const scriptPath = path.join(repoRoot, 'docs/portal/scripts/preview-feedback-wave.mjs');

async function createTempRoot() {
  const dir = await mkdtemp(path.join(tmpdir(), 'preview-feedback-wave-'));
  return {
    dir,
    logPath: path.join(dir, 'feedback_log.json'),
    artifactDir: path.join(dir, 'artifacts'),
    portalSummaryPath: path.join(dir, 'portal-summary.json'),
  };
}

test('generates wave summary, digest, and portal data from the shared log', async () => {
  const {dir, logPath, artifactDir, portalSummaryPath} = await createTempRoot();
  const wave = 'preview-20260301';
  const logPayload = {
    version: 1,
    entries: [
      {
        timestamp: '2026-03-01T10:00:00.000Z',
        wave,
        recipient: 'alice@example.com',
        event: 'invite-sent',
      },
      {
        timestamp: '2026-03-01T10:05:00.000Z',
        wave,
        recipient: 'bob@example.com',
        event: 'invite-sent',
      },
      {
        timestamp: '2026-03-01T10:10:00.000Z',
        wave,
        recipient: 'carol@example.com',
        event: 'invite-sent',
      },
      {
        timestamp: '2026-03-01T12:00:00.000Z',
        wave,
        recipient: 'alice@example.com',
        event: 'acknowledged',
      },
      {
        timestamp: '2026-03-02T15:00:00.000Z',
        wave,
        recipient: 'bob@example.com',
        event: 'acknowledged',
      },
      {
        timestamp: '2026-03-02T18:30:00.000Z',
        wave,
        recipient: 'alice@example.com',
        event: 'feedback-submitted',
        notes: 'docs-preview/test#1',
      },
      {
        timestamp: '2026-03-04T09:00:00.000Z',
        wave,
        recipient: 'alice@example.com',
        event: 'issue-opened',
        notes: 'DOCS-SORA-TEST',
      },
      {
        timestamp: '2026-03-05T08:00:00.000Z',
        wave,
        recipient: 'alice@example.com',
        event: 'access-revoked',
      },
      {
        timestamp: '2026-03-05T09:00:00.000Z',
        wave,
        recipient: 'bob@example.com',
        event: 'access-revoked',
      },
    ],
  };

  await writeFile(logPath, JSON.stringify(logPayload, null, 2));

  try {
    await execFileAsync('node', [
      scriptPath,
      '--wave',
      wave,
      '--log',
      logPath,
      '--artifact-dir',
      artifactDir,
      '--portal-summary-out',
      portalSummaryPath,
      '--dashboard',
      'https://grafana.example/wave',
      '--report-date',
      '2026-03-06',
      '--invite-start',
      '2026-03-01',
      '--invite-end',
      '2026-03-05',
      '--notes',
      'Beta invite wave',
    ], {
      cwd: repoRoot,
      env: {
        ...process.env,
        PREVIEW_FEEDBACK_NOW: '2026-03-06T00:00:00.000Z',
      },
    });

    const summaryPath = path.join(artifactDir, `${wave}-summary.json`);
    const digestPath = path.join(artifactDir, `${wave}-digest.md`);
    const summary = JSON.parse(await readFile(summaryPath, 'utf8'));
    assert.equal(summary.wave, wave);
    assert.equal(summary.invites.total, 3);
    assert.equal(summary.invites.open, 1);
    assert.equal(summary.acknowledgements, 2);
    assert.equal(summary.revocations, 2);
    assert.equal(summary.feedback_submitted, 1);
    assert.deepEqual(summary.issue_ids, ['DOCS-SORA-TEST']);
    assert.equal(summary.response_rate_percent, 33.3);
    assert.equal(summary.last_event_timestamp, '2026-03-05T09:00:00.000Z');
    assert.equal(summary.notes, 'Beta invite wave');

    const digest = await readFile(digestPath, 'utf8');
    assert.match(digest, /Wave preview-20260301 feedback digest/);
    assert.match(digest, /Reviewers invited: 3 \(open: 1\)/);
    assert.match(digest, /Feedback submissions: 1/);
    assert.match(digest, /Dashboard snapshot: https:\/\/grafana.example\/wave/);

    const portalData = JSON.parse(await readFile(portalSummaryPath, 'utf8'));
    assert.equal(portalData.source.endsWith('feedback_log.json'), true);
    const [waveRow] = portalData.waves.filter((entry) => entry.wave === wave);
    assert.ok(waveRow);
    assert.equal(waveRow.invites.total, 3);
    assert.equal(waveRow.invites.open, 1);
    assert.equal(waveRow.status, 'active');
    assert.equal(waveRow.responseRatePercent, 33.3);
  } finally {
    await rm(dir, {recursive: true, force: true});
  }
});
