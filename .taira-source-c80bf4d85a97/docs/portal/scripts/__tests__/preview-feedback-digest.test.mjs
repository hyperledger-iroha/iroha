import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {execFile} from 'node:child_process';
import {mkdtemp, readFile, rm, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import test from 'node:test';
import {promisify} from 'node:util';

const execFileAsync = promisify(execFile);
const repoRoot = fileURLToPath(new URL('../../../../', import.meta.url));
const scriptPath = join(repoRoot, 'docs/portal/scripts/preview-feedback-digest.mjs');

async function writeLog(entries) {
  const dir = await mkdtemp(join(tmpdir(), 'preview-digest-'));
  const logPath = join(dir, 'feedback_log.json');
  await writeFile(logPath, JSON.stringify({version: 1, entries}, null, 2), 'utf8');
  return {dir, logPath};
}

test('generates markdown digest with metrics and artefacts', async () => {
  const entries = [
    {
      wave: 'preview-wave',
      recipient: 'alice@example.com',
      event: 'invite-sent',
      timestamp: '2025-02-01T09:00:00.000Z',
    },
    {
      wave: 'preview-wave',
      recipient: 'alice@example.com',
      event: 'acknowledged',
      timestamp: '2025-02-01T09:05:00.000Z',
    },
    {
      wave: 'preview-wave',
      recipient: 'alice@example.com',
      event: 'feedback-submitted',
      timestamp: '2025-02-02T12:00:00.000Z',
    },
    {
      wave: 'preview-wave',
      recipient: 'alice@example.com',
      event: 'access-revoked',
      timestamp: '2025-02-05T00:00:00.000Z',
    },
  ];
  const {dir, logPath} = await writeLog(entries);
  const digestPath = join(dir, 'digest.md');
  const summaryPath = join(dir, 'preview-wave-summary.json');
  await writeFile(summaryPath, '{"sample":true}', 'utf8');
  const env = {...process.env};
  try {
    await execFileAsync(
      'node',
      [
        scriptPath,
        '--wave',
        'preview-wave',
        '--log',
        logPath,
        '--summary',
        summaryPath,
        '--out',
        digestPath,
        '--report-date',
        '2025-02-07',
        '--invite-start',
        '2025-02-01T00:00:00Z',
        '--invite-end',
        '2025-02-05T00:00:00Z',
        '--dashboard',
        'https://grafana.example/d/preview-wave',
      ],
      {cwd: repoRoot, env},
    );
    const content = await readFile(digestPath, 'utf8');
    assert.match(content, /Wave preview-wave feedback digest \(2025-02-07\)/);
    assert.match(content, /Invite window: 2025-02-01 → 2025-02-05/);
    assert.match(content, /Reviewers invited: 1 \(open: 0\)/);
    assert.match(content, /Feedback submissions: 1/);
    assert.match(content, /Issues opened: 0/);
    assert.match(content, /Response rate: 100%/);
    assert.match(content, /Acknowledgement latency avg: 0.08h/);
    assert.match(content, /Feedback latency avg: 27h/);
    assert.match(content, /\| invite-sent \| 1 \|/);
    assert.match(content, /\| access-revoked \| 1 \|/);
    assert.match(content, /Dashboard snapshot: https:\/\/grafana\.example\/d\/preview-wave/);
    const logSha = createHash('sha256')
      .update(await readFile(logPath))
      .digest('hex');
    assert.match(content, new RegExp(`sha256:${logSha}`));
  } finally {
    await rm(dir, {recursive: true, force: true});
  }
});

test('supports stdout when --out - is supplied', async () => {
  const entries = [
    {
      wave: 'preview-stdout',
      recipient: 'bob@example.com',
      event: 'invite-sent',
      timestamp: '2025-03-01T00:00:00.000Z',
    },
  ];
  const {dir, logPath} = await writeLog(entries);
  try {
    const {stdout} = await execFileAsync(
      'node',
      [
        scriptPath,
        '--wave',
        'preview-stdout',
        '--log',
        logPath,
        '--out',
        '-',
        '--report-date',
        '2025-03-02',
      ],
      {cwd: repoRoot},
    );
    assert.match(stdout, /Wave preview-stdout feedback digest \(2025-03-02\)/);
    assert.match(stdout, /Reviewers invited: 1 \(open: 1\)/);
  } finally {
    await rm(dir, {recursive: true, force: true});
  }
});

test('fails when the requested wave is not in the log', async () => {
  const entries = [
    {
      wave: 'preview-existing',
      recipient: 'ops@example.com',
      event: 'invite-sent',
      timestamp: '2025-03-05T00:00:00.000Z',
    },
  ];
  const {dir, logPath} = await writeLog(entries);
  try {
    await assert.rejects(
      execFileAsync(
        'node',
        [scriptPath, '--wave', 'preview-missing', '--log', logPath],
        {cwd: repoRoot},
      ),
      (error) => {
        assert.equal(error.code, 1);
        assert.match(error.stderr, /no entries found for wave "preview-missing"/);
        return true;
      },
    );
  } finally {
    await rm(dir, {recursive: true, force: true});
  }
});
