#!/usr/bin/env node

/**
 * Preview feedback logger for DOCS-SORA invite waves.
 *
 * Records invite/feedback events in a canonical JSON log so governance reviews
 * and roadmap checkpoints have reproducible evidence.
 *
 * Usage examples:
 *   node scripts/preview-feedback-log.mjs --wave preview-20250303 \
 *     --recipient alice@example.com --event invite-sent --notes "Seed wave"
 *
 *   node scripts/preview-feedback-log.mjs --summary --summary-json \
 *     --log artifacts/docs_portal_preview/feedback_log.json
 */

import {mkdir, readFile, writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import path from 'node:path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const isMainModule =
  process.argv[1] && path.resolve(process.argv[1]) === __filename;
const repoRoot = path.resolve(__dirname, '..', '..', '..');
const DEFAULT_LOG_PATH = path.join(
  repoRoot,
  'artifacts',
  'docs_portal_preview',
  'feedback_log.json',
);
const LOG_VERSION = 1;
const VALID_EVENTS = new Set([
  'invite-sent',
  'acknowledged',
  'feedback-submitted',
  'issue-opened',
  'access-revoked',
]);

function timestampToMillis(value) {
  if (!value) return null;
  const parsed = Date.parse(value);
  if (Number.isNaN(parsed)) {
    return null;
  }
  return parsed;
}

function diffHours(laterMillis, earlierMillis) {
  if (laterMillis == null || earlierMillis == null) return null;
  const delta = laterMillis - earlierMillis;
  if (!Number.isFinite(delta) || delta < 0) {
    return null;
  }
  return delta / 3_600_000;
}

function computeLatencyStats(samples) {
  if (!samples || samples.length === 0) {
    return {avg: null, p95: null};
  }
  const total = samples.reduce((sum, value) => sum + value, 0);
  const avg = Number((total / samples.length).toFixed(2));
  const sorted = [...samples].sort((a, b) => a - b);
  const index = Math.min(
    sorted.length - 1,
    Math.max(0, Math.ceil(sorted.length * 0.95) - 1),
  );
  const p95 = Number(sorted[index].toFixed(2));
  return {avg, p95};
}

function responseRatePercent(totalInvites, feedbackCount) {
  if (!totalInvites || totalInvites <= 0) {
    return null;
  }
  return Number(((feedbackCount / totalInvites) * 100).toFixed(1));
}

async function loadLogFile(logPath) {
  try {
    const raw = await readFile(logPath, 'utf8');
    const parsed = JSON.parse(raw);
    if (
      typeof parsed !== 'object' ||
      parsed === null ||
      parsed.version !== LOG_VERSION ||
      !Array.isArray(parsed.entries)
    ) {
      throw new Error('log file missing version/entries');
    }
    return parsed.entries;
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      return [];
    }
    throw error;
  }
}

async function writeLogFile(logPath, entries) {
  const payload = JSON.stringify(
    {
      version: LOG_VERSION,
      generated_at: new Date().toISOString(),
      entries,
    },
    null,
    2,
  );
  await mkdir(path.dirname(logPath), {recursive: true});
  await writeFile(logPath, payload, 'utf8');
}

function appendEntry(entries, entry) {
  const next = entries.concat(entry);
  next.sort((a, b) => {
    const lhs = a.timestamp ?? '';
    const rhs = b.timestamp ?? '';
    return lhs.localeCompare(rhs);
  });
  return next;
}

function createSummary(entries, filterWave) {
  const summary = new Map();
  const recipientState = new Map();

  for (const entry of entries) {
    const timestampMs = timestampToMillis(entry.timestamp);
    if (!entry.wave || !entry.event) continue;
    if (filterWave && entry.wave !== filterWave) continue;

    const waveInfo = summary.get(entry.wave) ?? {
      wave: entry.wave,
      eventCounts: {},
      totalEvents: 0,
      recipients: {
        totalInvites: 0,
        openRecipients: 0,
      },
      acknowledgements: 0,
      revocations: 0,
      responseRatePercent: null,
      lastTimestamp: undefined,
      latencySamples: {ack: [], feedback: []},
    };

    waveInfo.totalEvents += 1;
    waveInfo.eventCounts[entry.event] =
      (waveInfo.eventCounts[entry.event] ?? 0) + 1;
    if (
      !waveInfo.lastTimestamp ||
      (entry.timestamp && entry.timestamp > waveInfo.lastTimestamp)
    ) {
      waveInfo.lastTimestamp = entry.timestamp;
    }

    const recipientKey = `${entry.wave}::${entry.recipient ?? 'unknown'}`;
    const recipientInfo = recipientState.get(recipientKey) ?? {
      hasInvite: false,
      isClosed: false,
      firstInvite: null,
      firstAck: null,
      firstFeedback: null,
    };

    if (entry.event === 'invite-sent') {
      if (!recipientInfo.hasInvite) {
        recipientInfo.hasInvite = true;
        waveInfo.recipients.totalInvites += 1;
      }
      if (recipientInfo.firstInvite == null && timestampMs != null) {
        recipientInfo.firstInvite = timestampMs;
      }
    }
    if (entry.event === 'acknowledged') {
      waveInfo.acknowledgements += 1;
      if (recipientInfo.firstAck == null && timestampMs != null) {
        recipientInfo.firstAck = timestampMs;
        const deltaHours = diffHours(timestampMs, recipientInfo.firstInvite);
        if (deltaHours != null) {
          waveInfo.latencySamples.ack.push(deltaHours);
        }
      }
    }
    if (entry.event === 'access-revoked') {
      waveInfo.revocations += 1;
    }
    if (entry.event === 'access-revoked') {
      recipientInfo.isClosed = true;
    }
    if (entry.event === 'feedback-submitted') {
      if (recipientInfo.firstFeedback == null && timestampMs != null) {
        recipientInfo.firstFeedback = timestampMs;
        const deltaHours = diffHours(timestampMs, recipientInfo.firstInvite);
        if (deltaHours != null) {
          waveInfo.latencySamples.feedback.push(deltaHours);
        }
      }
    }

    recipientState.set(recipientKey, recipientInfo);
    summary.set(entry.wave, waveInfo);
  }

  // Compute open-recipient counts
  for (const [key, state] of recipientState.entries()) {
    const [wave] = key.split('::');
    const waveInfo = summary.get(wave);
    if (!waveInfo) continue;
    if (state.hasInvite && !state.isClosed) {
      waveInfo.recipients.openRecipients += 1;
    }
  }

  // Compute response rates once final invite/feedback counts are known
  for (const waveInfo of summary.values()) {
    const feedbackCount = waveInfo.eventCounts['feedback-submitted'] ?? 0;
    waveInfo.responseRatePercent = responseRatePercent(
      waveInfo.recipients.totalInvites,
      feedbackCount,
    );
    const ackStats = computeLatencyStats(waveInfo.latencySamples.ack);
    const feedbackStats = computeLatencyStats(waveInfo.latencySamples.feedback);
    waveInfo.ackLatencyHoursAvg = ackStats.avg;
    waveInfo.ackLatencyHoursP95 = ackStats.p95;
    waveInfo.feedbackLatencyHoursAvg = feedbackStats.avg;
    waveInfo.feedbackLatencyHoursP95 = feedbackStats.p95;
    delete waveInfo.latencySamples;
  }

  return Array.from(summary.values()).sort((a, b) =>
    a.wave.localeCompare(b.wave),
  );
}

function formatSummary(summary) {
  if (summary.length === 0) {
    return 'No preview feedback entries found for the requested selection.';
  }
  const lines = [];
  for (const item of summary) {
    lines.push(`Wave ${item.wave}`);
    lines.push(`  total events: ${item.totalEvents}`);
    lines.push(
      `  invites logged: ${item.recipients.totalInvites} (open: ${item.recipients.openRecipients})`,
    );
    lines.push(`  acknowledgements: ${item.acknowledgements ?? 0}`);
    lines.push(`  revocations: ${item.revocations ?? 0}`);
    const feedbackCount = item.eventCounts['feedback-submitted'] ?? 0;
    const issueCount = item.eventCounts['issue-opened'] ?? 0;
    const responseRate =
      item.responseRatePercent ??
      responseRatePercent(item.recipients.totalInvites, feedbackCount);
    lines.push(`  feedback submissions: ${feedbackCount}`);
    lines.push(`  issues opened: ${issueCount}`);
    if (responseRate !== null) {
      lines.push(`  response rate: ${responseRate}%`);
    }
    if (item.lastTimestamp) {
      lines.push(`  last event: ${item.lastTimestamp}`);
    }
    if (item.ackLatencyHoursAvg != null) {
      lines.push(
        `  ack latency avg: ${item.ackLatencyHoursAvg}h (p95: ${item.ackLatencyHoursP95 ?? 'n/a'}h)`,
      );
    }
    if (item.feedbackLatencyHoursAvg != null) {
      lines.push(
        `  feedback latency avg: ${item.feedbackLatencyHoursAvg}h (p95: ${item.feedbackLatencyHoursP95 ?? 'n/a'}h)`,
      );
    }
  }
  return `${lines.join('\n')}\n`;
}

function usage() {
  return `Usage:
  node scripts/preview-feedback-log.mjs --wave <label> --recipient <email> --event <event> [--notes "..."] [--log <path>]
  node scripts/preview-feedback-log.mjs --summary [--summary-json] [--filter-wave <label>] [--log <path>]

Events: ${Array.from(VALID_EVENTS).join(', ')}
Default log: ${path.relative(repoRoot, DEFAULT_LOG_PATH)}
`;
}

function parseArgs(argv) {
  const args = {
    logPath: DEFAULT_LOG_PATH,
    summary: false,
    summaryJson: false,
    filterWave: null,
    wave: null,
    recipient: null,
    event: null,
    notes: null,
  };
  for (let i = 2; i < argv.length; i += 1) {
    const token = argv[i];
    switch (token) {
      case '--help':
      case '-h':
        return {...args, help: true};
      case '--log':
        args.logPath = path.resolve(process.cwd(), argv[++i] ?? '');
        break;
      case '--wave':
        args.wave = argv[++i] ?? null;
        break;
      case '--recipient':
        args.recipient = argv[++i] ?? null;
        break;
      case '--event':
        args.event = argv[++i] ?? null;
        break;
      case '--notes':
        args.notes = argv[++i] ?? null;
        break;
      case '--summary':
        args.summary = true;
        break;
      case '--summary-json':
        args.summary = true;
        args.summaryJson = true;
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

async function runCli() {
  let parsed;
  try {
    parsed = parseArgs(process.argv);
  } catch (error) {
    console.error(`[preview-feedback-log] ${error.message}`);
    process.exit(1);
  }

  if (parsed.help) {
    process.stdout.write(usage());
    return;
  }

  if (parsed.summary) {
    const entries = await loadLogFile(parsed.logPath);
    const summary = createSummary(entries, parsed.filterWave);
    if (parsed.summaryJson) {
      process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
    } else {
      process.stdout.write(`${formatSummary(summary)}\n`);
    }
    return;
  }

  if (!parsed.wave || !parsed.recipient || !parsed.event) {
    console.error(
      '[preview-feedback-log] --wave, --recipient, and --event are required',
    );
    process.exit(1);
  }
  if (!VALID_EVENTS.has(parsed.event)) {
    console.error(
      `[preview-feedback-log] unsupported event "${parsed.event}"`,
    );
    process.exit(1);
  }

  const timestamp =
    process.env.PREVIEW_FEEDBACK_TIMESTAMP ?? new Date().toISOString();
  const entry = {
    timestamp,
    wave: parsed.wave,
    recipient: parsed.recipient,
    event: parsed.event,
  };
  if (parsed.notes) {
    entry.notes = parsed.notes;
  }

  const entries = await loadLogFile(parsed.logPath);
  const nextEntries = appendEntry(entries, entry);
  await writeLogFile(parsed.logPath, nextEntries);
  const relative = path.relative(process.cwd(), parsed.logPath);
  console.error(
    `[preview-feedback-log] recorded ${parsed.event} for ${parsed.recipient} (${parsed.wave}) in ${relative}`,
  );
}

if (isMainModule) {
  runCli().catch((error) => {
    console.error('[preview-feedback-log] failed:', error);
    process.exit(1);
  });
}

export {
  DEFAULT_LOG_PATH,
  LOG_VERSION,
  VALID_EVENTS,
  appendEntry,
  createSummary,
  loadLogFile,
  responseRatePercent,
  writeLogFile,
  usage,
};
