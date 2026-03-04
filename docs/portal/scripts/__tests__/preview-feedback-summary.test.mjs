import assert from 'node:assert/strict';
import {execFile} from 'node:child_process';
import {mkdtemp, readFile, rm, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import test from 'node:test';
import {promisify} from 'node:util';

const execFileAsync = promisify(execFile);
const repoRoot = fileURLToPath(new URL('../../../../', import.meta.url));
const scriptPath = join(repoRoot, 'docs/portal/scripts/preview-feedback-summary.mjs');

async function createTempLogWithEntries(entries) {
  const dir = await mkdtemp(join(tmpdir(), 'preview-feedback-summary-'));
  const logPath = join(dir, 'feedback_log.json');
  await writeFile(logPath, JSON.stringify({version: 1, entries}, null, 2), 'utf8');
  return {dir, logPath};
}

test('writes portal summary JSON', async () => {
  const entries = [
    {
      wave: 'preview-test',
      recipient: 'alice@example.com',
      event: 'invite-sent',
      timestamp: '2025-02-01T10:00:00.000Z',
    },
    {
      wave: 'preview-test',
      recipient: 'alice@example.com',
      event: 'acknowledged',
      timestamp: '2025-02-01T10:05:00.000Z',
    },
    {
      wave: 'preview-test',
      recipient: 'alice@example.com',
      event: 'feedback-submitted',
      timestamp: '2025-02-02T11:00:00.000Z',
    },
    {
      wave: 'preview-test',
      recipient: 'alice@example.com',
      event: 'access-revoked',
      timestamp: '2025-02-03T12:00:00.000Z',
    },
  ];
  const {dir, logPath} = await createTempLogWithEntries(entries);
  const outPath = join(dir, 'summary.json');
  const env = {
    ...process.env,
    PREVIEW_FEEDBACK_NOW: '2025-02-05T12:00:00.000Z',
  };
  try {
    await execFileAsync('node', [scriptPath, '--log', logPath, '--out', outPath], {
      cwd: repoRoot,
      env,
    });
    const payload = JSON.parse(await readFile(outPath, 'utf8'));
    assert.ok(payload.generatedAt, 'generatedAt missing');
    assert.equal(payload.source.endsWith('feedback_log.json'), true);
    assert.equal(payload.waves.length, 1);
    const [wave] = payload.waves;
    assert.equal(wave.wave, 'preview-test');
    assert.equal(wave.totalEvents, 4);
    assert.equal(wave.invites.total, 1);
    assert.equal(wave.invites.open, 0);
    assert.equal(wave.acknowledgements, 1);
    assert.equal(wave.feedbackCount, 1);
    assert.equal(wave.revocations, 1);
    assert.equal(wave.responseRatePercent, 100);
    assert.equal(wave.inviteCompletionPercent, 100);
    assert.equal(wave.ackRatePercent, 100);
    assert.equal(wave.feedbackRatePercent, 100);
    assert.equal(wave.revocationRatePercent, 100);
    assert.equal(wave.issueRatePercent, 0);
    assert.equal(wave.eventCounts['feedback-submitted'], 1);
    assert.equal(wave.lastEventTimestamp, '2025-02-03T12:00:00.000Z');
    assert.equal(wave.ackLatencyHoursAvg, 0.08);
    assert.equal(wave.ackLatencyHoursP95, 0.08);
    assert.equal(wave.feedbackLatencyHoursAvg, 25);
    assert.equal(wave.feedbackLatencyHoursP95, 25);
    assert.equal(wave.status, 'closed');
    assert.equal(wave.daysSinceLastEvent, 2);
  } finally {
    await rm(dir, {recursive: true, force: true});
  }
});

test('respects filter-wave flag', async () => {
  const events = [
    {wave: 'preview-a', recipient: 'a', event: 'invite-sent', timestamp: '2025-01-01T00:00:00.000Z'},
    {wave: 'preview-b', recipient: 'b', event: 'invite-sent', timestamp: '2025-01-02T00:00:00.000Z'},
  ];
  const {dir, logPath} = await createTempLogWithEntries(events);
  const outPath = join(dir, 'filtered.json');
  const env = {
    ...process.env,
    PREVIEW_FEEDBACK_NOW: '2025-01-05T00:00:00.000Z',
  };
  try {
    await execFileAsync(
      'node',
      [scriptPath, '--log', logPath, '--out', outPath, '--filter-wave', 'preview-b'],
      {cwd: repoRoot, env},
    );
    const payload = JSON.parse(await readFile(outPath, 'utf8'));
    assert.equal(payload.waves.length, 1);
    assert.equal(payload.waves[0].wave, 'preview-b');
  } finally {
    await rm(dir, {recursive: true, force: true});
  }
});

test('handles missing log by writing empty summary', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'preview-feedback-summary-missing-'));
  const logPath = join(dir, 'does-not-exist.json');
  const outPath = join(dir, 'empty.json');
  try {
    await execFileAsync('node', [scriptPath, '--log', logPath, '--out', outPath], {
      cwd: repoRoot,
      env: {...process.env, PREVIEW_FEEDBACK_NOW: '2025-02-01T00:00:00.000Z'},
    });
    const payload = JSON.parse(await readFile(outPath, 'utf8'));
    assert.equal(payload.waves.length, 0);
  } finally {
    await rm(dir, {recursive: true, force: true});
  }
});

test('exits with an error on unknown flags', async () => {
  await assert.rejects(
    execFileAsync('node', [scriptPath, '--unsupported'], {
      cwd: repoRoot,
      env: {...process.env, PREVIEW_FEEDBACK_NOW: '2025-02-01T00:00:00.000Z'},
    }),
    (error) => {
      assert.equal(error.code, 1);
      assert.match(error.stderr, /unknown argument: --unsupported/);
      return true;
    },
  );
});

test('computes wave status based on open invites and recency', async () => {
  const entries = [
    {
      wave: 'preview-active',
      recipient: 'a@example.com',
      event: 'invite-sent',
      timestamp: '2025-03-10T00:00:00.000Z',
    },
    {
      wave: 'preview-archived',
      recipient: 'b@example.com',
      event: 'invite-sent',
      timestamp: '2024-12-01T00:00:00.000Z',
    },
    {
      wave: 'preview-archived',
      recipient: 'b@example.com',
      event: 'access-revoked',
      timestamp: '2024-12-02T00:00:00.000Z',
    },
    {
      wave: 'preview-closed',
      recipient: 'c@example.com',
      event: 'invite-sent',
      timestamp: '2025-03-01T00:00:00.000Z',
    },
    {
      wave: 'preview-closed',
      recipient: 'c@example.com',
      event: 'access-revoked',
      timestamp: '2025-03-05T00:00:00.000Z',
    },
    {
      wave: 'preview-pending',
      recipient: 'ops@example.com',
      event: 'issue-opened',
      timestamp: '2025-03-01T00:00:00.000Z',
    },
  ];
  const {dir, logPath} = await createTempLogWithEntries(entries);
  const outPath = join(dir, 'status.json');
  const env = {
    ...process.env,
    PREVIEW_FEEDBACK_NOW: '2025-03-15T00:00:00.000Z',
  };
  try {
    await execFileAsync('node', [scriptPath, '--log', logPath, '--out', outPath], {
      cwd: repoRoot,
      env,
    });
    const payload = JSON.parse(await readFile(outPath, 'utf8'));
    const waves = Object.fromEntries(payload.waves.map((wave) => [wave.wave, wave]));
    assert.equal(waves['preview-active'].status, 'active');
    assert.equal(waves['preview-active'].daysSinceLastEvent, null);
    assert.equal(waves['preview-closed'].status, 'closed');
    assert.equal(waves['preview-closed'].daysSinceLastEvent, 10);
    assert.equal(waves['preview-archived'].status, 'archived');
    assert.ok((waves['preview-archived'].daysSinceLastEvent ?? 0) >= 100);
    assert.equal(waves['preview-pending'].status, 'pending');
    assert.equal(waves['preview-pending'].daysSinceLastEvent, null);
    assert.equal(waves['preview-pending'].inviteCompletionPercent, null);
    assert.equal(waves['preview-pending'].ackRatePercent, null);
  } finally {
    await rm(dir, {recursive: true, force: true});
  }
});

test('rejects invalid PREVIEW_FEEDBACK_NOW overrides', async () => {
  await assert.rejects(
    execFileAsync('node', [scriptPath], {
      cwd: repoRoot,
      env: {...process.env, PREVIEW_FEEDBACK_NOW: 'not-a-date'},
    }),
    (error) => {
      assert.equal(error.code, 1);
      assert.match(error.stderr, /PREVIEW_FEEDBACK_NOW must be a valid ISO-8601 timestamp/);
      return true;
    },
  );
});
