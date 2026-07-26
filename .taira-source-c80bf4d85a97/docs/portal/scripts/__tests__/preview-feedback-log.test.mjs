import assert from 'node:assert/strict';
import {execFile} from 'node:child_process';
import {mkdtemp, readFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import test from 'node:test';
import {promisify} from 'node:util';

const execFileAsync = promisify(execFile);
const repoRoot = fileURLToPath(new URL('../../../../', import.meta.url));
const scriptPath = join(repoRoot, 'docs/portal/scripts/preview-feedback-log.mjs');

async function createTempLog() {
  const dir = await mkdtemp(join(tmpdir(), 'preview-feedback-log-'));
  return {dir, logPath: join(dir, 'feedback_log.json')};
}

test('records preview events in the log', async () => {
  const {dir, logPath} = await createTempLog();
  try {
    const baseEnv = {
      ...process.env,
      PREVIEW_FEEDBACK_TIMESTAMP: '2025-03-01T00:00:00.000Z',
    };
    await execFileAsync('node', [
      scriptPath,
      '--log',
      logPath,
      '--wave',
      'preview-20250301',
      '--recipient',
      'alice@example.com',
      '--event',
      'invite-sent',
      '--notes',
      'initial invite',
    ], {env: baseEnv, cwd: repoRoot});

    await execFileAsync('node', [
      scriptPath,
      '--log',
      logPath,
      '--wave',
      'preview-20250301',
      '--recipient',
      'alice@example.com',
      '--event',
      'feedback-submitted',
    ], {
      env: {
        ...process.env,
        PREVIEW_FEEDBACK_TIMESTAMP: '2025-03-02T00:00:00.000Z',
      },
      cwd: repoRoot,
    });

    const contents = JSON.parse(await readFile(logPath, 'utf8'));
    assert.equal(contents.version, 1);
    assert.equal(contents.entries.length, 2);
    assert.deepEqual(contents.entries[0], {
      timestamp: '2025-03-01T00:00:00.000Z',
      wave: 'preview-20250301',
      recipient: 'alice@example.com',
      event: 'invite-sent',
      notes: 'initial invite',
    });
    assert.deepEqual(contents.entries[1], {
      timestamp: '2025-03-02T00:00:00.000Z',
      wave: 'preview-20250301',
      recipient: 'alice@example.com',
      event: 'feedback-submitted',
    });
  } finally {
    await rm(dir, {recursive: true, force: true});
  }
});

test('summary emits per-wave aggregates', async () => {
  const {dir, logPath} = await createTempLog();
  try {
    const events = [
      {
        time: '2025-03-05T10:00:00.000Z',
        event: 'invite-sent',
        recipient: 'alice@example.com',
      },
      {
        time: '2025-03-05T11:00:00.000Z',
        event: 'invite-sent',
        recipient: 'bob@example.com',
      },
      {
        time: '2025-03-06T09:30:00.000Z',
        event: 'acknowledged',
        recipient: 'alice@example.com',
      },
      {
        time: '2025-03-07T09:00:00.000Z',
        event: 'feedback-submitted',
        recipient: 'alice@example.com',
      },
      {
        time: '2025-03-09T18:00:00.000Z',
        event: 'access-revoked',
        recipient: 'alice@example.com',
      },
    ];

    for (const entry of events) {
      await execFileAsync('node', [
        scriptPath,
        '--log',
        logPath,
        '--wave',
        'preview-20250305',
        '--recipient',
        entry.recipient,
        '--event',
        entry.event,
      ], {
        env: {...process.env, PREVIEW_FEEDBACK_TIMESTAMP: entry.time},
        cwd: repoRoot,
      });
    }

    const {stdout} = await execFileAsync('node', [
      scriptPath,
      '--log',
      logPath,
      '--summary-json',
    ], {cwd: repoRoot});
    const summary = JSON.parse(stdout);
    assert.equal(summary.length, 1);
    const [waveSummary] = summary;
    assert.equal(waveSummary.wave, 'preview-20250305');
    assert.equal(waveSummary.recipients.totalInvites, 2);
    assert.equal(waveSummary.recipients.openRecipients, 1);
    assert.equal(waveSummary.acknowledgements, 1);
    assert.equal(waveSummary.revocations, 1);
    assert.equal(waveSummary.eventCounts['feedback-submitted'], 1);
    assert.equal(waveSummary.eventCounts['invite-sent'], 2);
    assert.equal(waveSummary.responseRatePercent, 50);
    assert.equal(waveSummary.lastTimestamp, '2025-03-09T18:00:00.000Z');
    assert.equal(waveSummary.ackLatencyHoursAvg, 23.5);
    assert.equal(waveSummary.ackLatencyHoursP95, 23.5);
    assert.equal(waveSummary.feedbackLatencyHoursAvg, 47);
    assert.equal(waveSummary.feedbackLatencyHoursP95, 47);
  } finally {
    await rm(dir, {recursive: true, force: true});
  }
});

test('text summary reports acknowledgements and response rate', async () => {
  const {dir, logPath} = await createTempLog();
  try {
    const events = [
      {time: '2025-04-01T08:00:00.000Z', event: 'invite-sent', recipient: 'a@example.com'},
      {time: '2025-04-01T09:00:00.000Z', event: 'acknowledged', recipient: 'a@example.com'},
      {time: '2025-04-02T12:00:00.000Z', event: 'feedback-submitted', recipient: 'a@example.com'},
    ];
    for (const entry of events) {
      await execFileAsync('node', [
        scriptPath,
        '--log',
        logPath,
        '--wave',
        'preview-20250401',
        '--recipient',
        entry.recipient,
        '--event',
        entry.event,
      ], {
        env: {...process.env, PREVIEW_FEEDBACK_TIMESTAMP: entry.time},
        cwd: repoRoot,
      });
    }

    const {stdout} = await execFileAsync(
      'node',
      [scriptPath, '--log', logPath, '--summary', '--filter-wave', 'preview-20250401'],
      {cwd: repoRoot},
    );

    assert.match(stdout, /acknowledgements: 1/);
    assert.match(stdout, /response rate: 100(?:\.0+)?%/);
    assert.match(stdout, /ack latency avg: 1(?:\.0+)?h \(p95: 1(?:\.0+)?h\)/);
    assert.match(stdout, /feedback latency avg: 28(?:\.0+)?h \(p95: 28(?:\.0+)?h\)/);
  } finally {
    await rm(dir, {recursive: true, force: true});
  }
});
