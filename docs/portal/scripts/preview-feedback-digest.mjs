#!/usr/bin/env node

/**
 * Generate a Markdown digest for a DOCS-SORA preview invite wave.
 *
 * Usage:
 *   node scripts/preview-feedback-digest.mjs \
 *     --wave preview-20250303 \
 *     --log artifacts/docs_portal_preview/feedback_log.json \
 *     --summary artifacts/docs_portal_preview/preview-20250303-summary.json \
 *     --out artifacts/docs_portal_preview/preview-20250303-digest.md
 */

import {createHash} from 'node:crypto';
import {mkdir, readFile, writeFile} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

import {
  DEFAULT_LOG_PATH,
  createSummary,
  loadLogFile,
  responseRatePercent as computeResponseRate,
} from './preview-feedback-log.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const isMainModule =
  process.argv[1] && path.resolve(process.argv[1]) === __filename;
const repoRoot = path.resolve(__dirname, '..', '..', '..');
const DEFAULT_ARTIFACT_DIR = path.join(
  repoRoot,
  'artifacts',
  'docs_portal_preview',
);

function usage() {
  const relativeLog = path.relative(repoRoot, DEFAULT_LOG_PATH);
  return `Usage:
  node scripts/preview-feedback-digest.mjs --wave <label> [--log <path>] [--summary <path>] [--out <path>|-] [--invite-start <iso>] [--invite-end <iso>] [--report-date <YYYY-MM-DD>] [--dashboard <url>] [--action \"item\" ...]

Defaults:
  log: ${relativeLog}
  summary: <artifact-dir>/<wave>-summary.json (if present)
  out: <artifact-dir>/<wave>-digest.md
`;
}

function sanitizeWaveLabel(label) {
  return label.replace(/[^A-Za-z0-9._-]+/g, '-');
}

function defaultDigestPath(wave) {
  return path.join(DEFAULT_ARTIFACT_DIR, `${sanitizeWaveLabel(wave)}-digest.md`);
}

function defaultSummaryPath(wave) {
  return path.join(
    DEFAULT_ARTIFACT_DIR,
    `${sanitizeWaveLabel(wave)}-summary.json`,
  );
}

function toIsoDateString(value) {
  if (!value) return null;
  const parsed = new Date(value);
  if (!Number.isFinite(parsed.getTime())) {
    return null;
  }
  return parsed.toISOString().slice(0, 10);
}

async function computeSha256(filePath) {
  try {
    const contents = await readFile(filePath);
    const hash = createHash('sha256');
    hash.update(contents);
    return hash.digest('hex');
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      return null;
    }
    throw error;
  }
}

async function ensureParentDir(outPath) {
  if (!outPath || outPath === '-' || outPath === '/dev/stdout') {
    return;
  }
  const dir = path.dirname(outPath);
  await mkdir(dir, {recursive: true});
}

function deriveInviteWindow(entries, waveSummary, overrides) {
  const inviteTimestamps = entries
    .filter((entry) => entry.event === 'invite-sent' && entry.timestamp)
    .map((entry) => entry.timestamp);
  const earliestInvite =
    inviteTimestamps.length > 0
      ? inviteTimestamps.reduce((minTs, ts) =>
          ts < minTs ? ts : minTs,
        )
      : null;
  const start =
    overrides.inviteStart ??
    toIsoDateString(earliestInvite) ??
    '<invite-start>';
  const end =
    overrides.inviteEnd ??
    toIsoDateString(waveSummary?.lastTimestamp) ??
    '<invite-end>';
  return `${start} → ${end}`;
}

function formatLatency(label, avg, p95) {
  if (avg == null) {
    return null;
  }
  const suffix = p95 == null ? '' : ` (p95: ${p95}h)`;
  return `${label}: ${avg}h${suffix}`;
}

function renderDigest({
  wave,
  reportDate,
  inviteWindow,
  summary,
  logInfo,
  summaryInfo,
  dashboardLink,
  actions,
}) {
  const actionItems = Array.isArray(actions) ? actions : [];
  const feedbackCount =
    summary.eventCounts?.['feedback-submitted'] ?? summary.feedbackCount ?? 0;
  const issueCount =
    summary.eventCounts?.['issue-opened'] ?? summary.issuesCount ?? 0;
  const acknowledgements =
    summary.acknowledgements ?? summary.eventCounts?.acknowledged ?? 0;
  const revocations =
    summary.revocations ?? summary.eventCounts?.['access-revoked'] ?? 0;
  const invitesTotal =
    summary.invites?.total ??
    summary.recipients?.totalInvites ??
    0;
  const invitesOpen =
    summary.invites?.open ?? summary.recipients?.openRecipients ?? 0;
  const responseRateValue =
    summary.responseRatePercent ??
    computeResponseRate(invitesTotal, feedbackCount);
  const responseRate =
    responseRateValue != null ? `${responseRateValue}%` : 'n/a';
  const ackLatency = formatLatency(
    'Acknowledgement latency avg',
    summary.ackLatencyHoursAvg ?? null,
    summary.ackLatencyHoursP95 ?? null,
  );
  const feedbackLatency = formatLatency(
    'Feedback latency avg',
    summary.feedbackLatencyHoursAvg ?? null,
    summary.feedbackLatencyHoursP95 ?? null,
  );

  const metricsLines = [
    `## Wave ${wave} feedback digest (${reportDate})`,
    `- Invite window: ${inviteWindow}`,
    `- Reviewers invited: ${invitesTotal} (open: ${invitesOpen})`,
    `- Feedback submissions: ${feedbackCount}`,
    `- Issues opened: ${issueCount}`,
    `- Response rate: ${responseRate}`,
    `- Latest event timestamp: ${summary.lastTimestamp ?? '—'}`,
  ];
  if (ackLatency) {
    metricsLines.push(`- ${ackLatency}`);
  }
  if (feedbackLatency) {
    metricsLines.push(`- ${feedbackLatency}`);
  }

  const eventTableLines = [
    '',
    '| Event | Count |',
    '| --- | --- |',
  ];
  const eventKeys = [
    'invite-sent',
    'acknowledged',
    'feedback-submitted',
    'issue-opened',
    'access-revoked',
  ];
  for (const key of eventKeys) {
    const count = summary.eventCounts?.[key] ?? 0;
    eventTableLines.push(`| ${key} | ${count} |`);
  }

  const planningTable = [
    '',
    '| Category | Details | Owner / Follow-up |',
    '| --- | --- | --- |',
    '| Highlights | _Add reviewer kudos or UX notes_ | _Owner + ETA_ |',
    '| Blocking findings | _Link critical issues / tickets_ | _Owner_ |',
    '| Minor polish items | _Group docs/UI tweaks_ | _Owner_ |',
    '| Telemetry anomalies | _Reference Grafana/Alertmanager links_ | _Owner_ |',
    '',
    '## Actions',
  ];
  if (actionItems.length === 0) {
    planningTable.push('1. No actions captured; append follow-ups below.');
  } else {
    actionItems.forEach((item, idx) => {
      planningTable.push(`${idx + 1}. ${item}`);
    });
  }
  planningTable.push(
    '',
    '## Artefacts',
    `- Feedback log: \`${logInfo.relativePath}\`${
      logInfo.sha ? ` (sha256:${logInfo.sha})` : ''
    }`,
  );
  if (summaryInfo) {
    planningTable.push(
      `- Wave summary: \`${summaryInfo.relativePath}\`${
        summaryInfo.sha ? ` (sha256:${summaryInfo.sha})` : ''
      }`,
    );
  }
  planningTable.push(
    `- Dashboard snapshot: ${dashboardLink ?? '_Add link to Grafana/Probe_'}`,
  );

  return `${[
    ...metricsLines,
    ...eventTableLines,
    ...planningTable,
    '',
    'Generated via `preview-feedback-digest` — update the placeholder sections before attaching the digest to governance packets.',
    '',
  ].join('\n')}`;
}

function parseArgs(argv) {
  const args = {
    logPath: DEFAULT_LOG_PATH,
    summaryPath: null,
    outPath: null,
    inviteStart: null,
    inviteEnd: null,
    reportDate: null,
    dashboardLink: null,
    wave: null,
    actions: [],
  };
  for (let i = 2; i < argv.length; i += 1) {
    const token = argv[i];
    switch (token) {
      case '--help':
      case '-h':
        return {...args, help: true};
      case '--wave':
        args.wave = argv[++i] ?? null;
        break;
      case '--log':
        args.logPath = path.resolve(process.cwd(), argv[++i] ?? '');
        break;
      case '--summary':
        args.summaryPath = path.resolve(process.cwd(), argv[++i] ?? '');
        break;
      case '--out':
        args.outPath = argv[++i] ?? null;
        if (args.outPath && args.outPath !== '-') {
          args.outPath = path.resolve(process.cwd(), args.outPath);
        }
        break;
      case '--invite-start':
        args.inviteStart = toIsoDateString(argv[++i] ?? null);
        break;
      case '--invite-end':
        args.inviteEnd = toIsoDateString(argv[++i] ?? null);
        break;
      case '--report-date':
        args.reportDate = toIsoDateString(argv[++i] ?? null);
        break;
      case '--dashboard':
        args.dashboardLink = argv[++i] ?? null;
        break;
      case '--action': {
        const item = argv[++i];
        if (item) {
          args.actions.push(item);
        }
        break;
      }
      default:
        throw new Error(`unknown argument: ${token}`);
    }
  }
  return args;
}

async function run() {
  let parsed;
  try {
    parsed = parseArgs(process.argv);
  } catch (error) {
    console.error(`[preview-feedback-digest] ${error.message}`);
    process.exit(1);
  }

  if (parsed?.help) {
    process.stdout.write(usage());
    return;
  }
  if (!parsed.wave) {
    console.error('[preview-feedback-digest] --wave is required');
    process.exit(1);
  }

  const logPath = parsed.logPath ?? DEFAULT_LOG_PATH;
  const entries = await loadLogFile(logPath);
  const summary = createSummary(entries, parsed.wave);
  if (summary.length === 0) {
    console.error(
      `[preview-feedback-digest] no entries found for wave "${parsed.wave}"`,
    );
    process.exit(1);
  }
  const waveSummary = summary[0];
  const waveEntries = entries.filter((entry) => entry.wave === parsed.wave);
  const inviteWindow = deriveInviteWindow(waveEntries, waveSummary, {
    inviteStart: parsed.inviteStart,
    inviteEnd: parsed.inviteEnd,
  });

  const reportDate =
    parsed.reportDate ?? toIsoDateString(new Date().toISOString());
  const outPath =
    parsed.outPath && parsed.outPath !== '-'
      ? parsed.outPath
      : parsed.outPath === '-'
        ? '-'
        : defaultDigestPath(parsed.wave);
  let summaryPath = parsed.summaryPath ?? null;
  let summarySha = null;
  if (!summaryPath) {
    const defaultPath = defaultSummaryPath(parsed.wave);
    const detectedSha = await computeSha256(defaultPath);
    if (detectedSha) {
      summaryPath = defaultPath;
      summarySha = detectedSha;
    }
  }
  if (summaryPath && !summarySha && summaryPath !== logPath) {
    summarySha = await computeSha256(summaryPath);
  }

  const logSha = await computeSha256(logPath);

  const content = renderDigest({
    wave: parsed.wave,
    reportDate: reportDate ?? '<report-date>',
    inviteWindow,
    summary: waveSummary,
    logInfo: {
      relativePath: path.relative(repoRoot, logPath),
      sha: logSha,
    },
    summaryInfo: summaryPath
      ? {
          relativePath: path.relative(repoRoot, summaryPath),
          sha: summarySha,
        }
      : null,
    dashboardLink: parsed.dashboardLink,
    actions: parsed.actions,
  });

  if (outPath === '-') {
    process.stdout.write(content);
    return;
  }

  await ensureParentDir(outPath);
  await writeFile(outPath, content, 'utf8');
  const relativeOut = path.relative(process.cwd(), outPath);
  process.stderr.write(
    `[preview-feedback-digest] wrote digest to ${relativeOut}\n`,
  );
}

if (isMainModule) {
  run().catch((error) => {
    console.error('[preview-feedback-digest] failed:', error);
    process.exit(1);
  });
}

export {
  computeSha256,
  defaultDigestPath,
  defaultSummaryPath,
  deriveInviteWindow,
  renderDigest,
  sanitizeWaveLabel,
};
