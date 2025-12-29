#!/usr/bin/env node

/**
 * Generate portal-ready preview feedback summaries from the shared log.
 *
 * Usage:
 *   node scripts/preview-feedback-summary.mjs \
 *     --log artifacts/docs_portal_preview/feedback_log.json \
 *     --out docs/portal/src/data/previewFeedbackSummary.json
 */

import {mkdir, writeFile} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

import {
  DEFAULT_LOG_PATH,
  createSummary,
  loadLogFile,
  responseRatePercent,
} from './preview-feedback-log.mjs';

const MS_PER_DAY = 86_400_000;
const ARCHIVE_THRESHOLD_DAYS = 14;

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const isMainModule =
  process.argv[1] && path.resolve(process.argv[1]) === __filename;
const repoRoot = path.resolve(__dirname, '..', '..', '..');
const DEFAULT_OUTPUT_PATH = path.join(
  repoRoot,
  'docs/portal/src/data/previewFeedbackSummary.json',
);

function buildPortalSummaryPayload(entries, logPath, filterWave, now = resolveNow()) {
  const summary = createSummary(entries, filterWave);
  return {
    generatedAt: now.toISOString(),
    source: path.relative(repoRoot, logPath),
    waves: normalizeSummary(summary, now),
  };
}

function usage() {
  return `Usage:
  node scripts/preview-feedback-summary.mjs [--log <path>] [--out <path>] [--filter-wave <label>]

Defaults:
  log: ${path.relative(repoRoot, DEFAULT_LOG_PATH)}
  out: ${path.relative(repoRoot, DEFAULT_OUTPUT_PATH)}
`;
}

function parseArgs(argv) {
  const args = {
    logPath: DEFAULT_LOG_PATH,
    outPath: DEFAULT_OUTPUT_PATH,
    filterWave: null,
    help: false,
  };
  for (let i = 2; i < argv.length; i += 1) {
    const token = argv[i];
    switch (token) {
      case '--help':
      case '-h':
        args.help = true;
        break;
      case '--log':
        args.logPath = path.resolve(process.cwd(), argv[++i] ?? '');
        break;
      case '--out':
        args.outPath = path.resolve(process.cwd(), argv[++i] ?? '');
        break;
      case '--filter-wave':
        args.filterWave = argv[++i] ?? null;
        break;
      default:
        throw new Error(`unknown argument: ${token}`);
    }
  }
  return args;
}

function normalizeSummary(summary, now) {
  return summary.map((item) => {
    const invitesTotal = item.recipients?.totalInvites ?? 0;
    const invitesOpen = item.recipients?.openRecipients ?? 0;
    const eventCounts = item.eventCounts ?? {};
    const acknowledgements = item.acknowledgements ?? eventCounts['acknowledged'] ?? 0;
    const revocations = item.revocations ?? eventCounts['access-revoked'] ?? 0;
    const feedback = eventCounts['feedback-submitted'] ?? 0;
    const issues = eventCounts['issue-opened'] ?? 0;
    const responseRate =
      item.responseRatePercent ?? responseRatePercent(invitesTotal, feedback);
    const completionPercent = percentOf(invitesTotal - invitesOpen, invitesTotal);
    const ackRatePercent = percentOf(acknowledgements, invitesTotal);
    const feedbackRatePercent = percentOf(feedback, invitesTotal);
    const revocationRatePercent = percentOf(revocations, invitesTotal);
    const issueRatePercent = percentOf(issues, invitesTotal);
    const {status, daysSinceLastEvent} = deriveWaveStatus(
      {
        invitesTotal,
        invitesOpen,
        lastEventTimestamp: item.lastTimestamp,
      },
      now,
    );
    return {
      wave: item.wave,
      totalEvents: item.totalEvents,
      invites: {
        total: invitesTotal,
        open: invitesOpen,
      },
      status,
      daysSinceLastEvent,
      acknowledgements,
      revocations,
      feedbackCount: feedback,
      issuesCount: issues,
      responseRatePercent: responseRate,
      inviteCompletionPercent: completionPercent,
      ackRatePercent,
      feedbackRatePercent,
      revocationRatePercent,
      issueRatePercent,
      lastEventTimestamp: item.lastTimestamp ?? null,
      eventCounts,
      ackLatencyHoursAvg: item.ackLatencyHoursAvg ?? null,
      ackLatencyHoursP95: item.ackLatencyHoursP95 ?? null,
      feedbackLatencyHoursAvg: item.feedbackLatencyHoursAvg ?? null,
      feedbackLatencyHoursP95: item.feedbackLatencyHoursP95 ?? null,
    };
  });
}

function percentOf(numerator, denominator) {
  if (
    numerator == null ||
    !Number.isFinite(numerator) ||
    !denominator ||
    denominator <= 0
  ) {
    return null;
  }
  return Number(((numerator / denominator) * 100).toFixed(1));
}

function deriveWaveStatus(waveInfo, now) {
  const invitesTotal = waveInfo.invitesTotal ?? 0;
  const invitesOpen = waveInfo.invitesOpen ?? 0;
  if (invitesTotal === 0) {
    return {status: 'pending', daysSinceLastEvent: null};
  }
  if (invitesOpen > 0) {
    return {status: 'active', daysSinceLastEvent: null};
  }
  const lastTimestamp = waveInfo.lastEventTimestamp;
  if (!lastTimestamp) {
    return {status: 'closed', daysSinceLastEvent: null};
  }
  const lastMillis = Date.parse(lastTimestamp);
  if (!Number.isFinite(lastMillis)) {
    return {status: 'closed', daysSinceLastEvent: null};
  }
  const diffMs = Math.max(0, now.getTime() - lastMillis);
  const daysSince = Math.floor(diffMs / MS_PER_DAY);
  if (daysSince >= ARCHIVE_THRESHOLD_DAYS) {
    return {status: 'archived', daysSinceLastEvent: daysSince};
  }
  return {status: 'closed', daysSinceLastEvent: daysSince};
}

function resolveNow() {
  const override = process.env.PREVIEW_FEEDBACK_NOW;
  if (!override) {
    return new Date();
  }
  const parsed = new Date(override);
  if (!Number.isFinite(parsed.getTime())) {
    throw new Error(
      'PREVIEW_FEEDBACK_NOW must be a valid ISO-8601 timestamp when provided',
    );
  }
  return parsed;
}

async function ensureParentDir(filePath) {
  const dir = path.dirname(filePath);
  if (!dir || dir === '.' || dir === '/') {
    return;
  }
  await mkdir(dir, {recursive: true});
}

async function run() {
  let parsed;
  try {
    parsed = parseArgs(process.argv);
  } catch (error) {
    console.error(`[preview-feedback-summary] ${error.message}`);
    process.exit(1);
  }

  if (parsed.help) {
    process.stdout.write(usage());
    return;
  }

  let entries;
  try {
    entries = await loadLogFile(parsed.logPath);
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      entries = [];
    } else {
      throw error;
    }
  }
  const payload = buildPortalSummaryPayload(
    entries,
    parsed.logPath,
    parsed.filterWave,
    resolveNow(),
  );

  const rendered = `${JSON.stringify(payload, null, 2)}\n`;
  await ensureParentDir(parsed.outPath);
  await writeFile(parsed.outPath, rendered, 'utf8');
  const relative = path.relative(process.cwd(), parsed.outPath);
  process.stderr.write(
    `[preview-feedback-summary] wrote summary for ${payload.waves.length} wave(s) to ${relative}\n`,
  );
}

if (isMainModule) {
  run().catch((error) => {
    console.error('[preview-feedback-summary] failed:', error);
    process.exit(1);
  });
}

export {
  buildPortalSummaryPayload,
  DEFAULT_OUTPUT_PATH,
  normalizeSummary,
  resolveNow,
};
