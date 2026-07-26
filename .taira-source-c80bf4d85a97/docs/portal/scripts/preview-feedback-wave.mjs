#!/usr/bin/env node

/**
 * End-to-end helper for DOCS-SORA invite waves.
 *
 * Given a wave label and the shared feedback log, this script generates:
 * - per-wave summary JSON (`<artifact-dir>/<wave>-summary.json`)
 * - Markdown digest (`<artifact-dir>/<wave>-digest.md`)
 * - optional portal data refresh (`docs/portal/src/data/previewFeedbackSummary.json`)
 *
 * Usage:
 *   node scripts/preview-feedback-wave.mjs --wave preview-20260218 \
 *     [--log artifacts/docs_portal_preview/feedback_log.json] \
 *     [--artifact-dir artifacts/docs_portal_preview] \
 *     [--summary-out <path>] [--digest-out <path>] \
 *     [--dashboard <url>] [--invite-start <ISO-date>] [--invite-end <ISO-date>] \
 *     [--report-date <ISO-date>] [--notes <text>] \
 *     [--portal-summary-out <path>] [--no-portal-data]
 */

import {mkdir, writeFile} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

import {
  DEFAULT_LOG_PATH,
  createSummary,
  loadLogFile,
} from './preview-feedback-log.mjs';
import {
  buildPortalSummaryPayload,
  DEFAULT_OUTPUT_PATH as DEFAULT_PORTAL_SUMMARY_PATH,
  normalizeSummary,
  resolveNow,
} from './preview-feedback-summary.mjs';
import {
  computeSha256,
  deriveInviteWindow,
  renderDigest,
  sanitizeWaveLabel,
} from './preview-feedback-digest.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..', '..', '..');
const DEFAULT_ARTIFACT_DIR = path.join(repoRoot, 'artifacts', 'docs_portal_preview');

function usage() {
  const relativeLog = path.relative(repoRoot, DEFAULT_LOG_PATH);
  return `Usage:
  node scripts/preview-feedback-wave.mjs --wave <label> \\
    [--log <path>] [--artifact-dir <dir>] [--summary-out <path>] [--digest-out <path>] \\
    [--dashboard <url>] [--invite-start <ISO-date>] [--invite-end <ISO-date>] \\
    [--report-date <ISO-date>] [--notes <text>] [--portal-summary-out <path>] [--no-portal-data]

Defaults:
  log: ${relativeLog}
  artifact dir: ${path.relative(repoRoot, DEFAULT_ARTIFACT_DIR)}
  summary-out: <artifact-dir>/<wave>-summary.json
  digest-out: <artifact-dir>/<wave>-digest.md
  portal data: ${path.relative(repoRoot, DEFAULT_PORTAL_SUMMARY_PATH)}
`;
}

function parseArgs(argv) {
  const args = {
    wave: null,
    logPath: DEFAULT_LOG_PATH,
    artifactDir: DEFAULT_ARTIFACT_DIR,
    summaryOut: null,
    digestOut: null,
    inviteStart: null,
    inviteEnd: null,
    reportDate: null,
    dashboardLink: null,
    notes: null,
    portalSummaryOut: DEFAULT_PORTAL_SUMMARY_PATH,
    updatePortalData: true,
    help: false,
  };

  for (let i = 2; i < argv.length; i += 1) {
    const token = argv[i];
    switch (token) {
      case '--help':
      case '-h':
        args.help = true;
        break;
      case '--wave':
        args.wave = argv[++i] ?? null;
        break;
      case '--log':
        args.logPath = path.resolve(process.cwd(), argv[++i] ?? '');
        break;
      case '--artifact-dir':
        args.artifactDir = path.resolve(process.cwd(), argv[++i] ?? '');
        break;
      case '--summary-out':
        args.summaryOut = path.resolve(process.cwd(), argv[++i] ?? '');
        break;
      case '--digest-out':
        args.digestOut = argv[++i] ?? null;
        if (args.digestOut && args.digestOut !== '-') {
          args.digestOut = path.resolve(process.cwd(), args.digestOut);
        }
        break;
      case '--invite-start':
        args.inviteStart = argv[++i] ?? null;
        break;
      case '--invite-end':
        args.inviteEnd = argv[++i] ?? null;
        break;
      case '--report-date':
        args.reportDate = argv[++i] ?? null;
        break;
      case '--dashboard':
        args.dashboardLink = argv[++i] ?? null;
        break;
      case '--notes':
        args.notes = argv[++i] ?? null;
        break;
      case '--portal-summary-out':
        args.portalSummaryOut = path.resolve(process.cwd(), argv[++i] ?? '');
        break;
      case '--no-portal-data':
        args.updatePortalData = false;
        break;
      default:
        throw new Error(`unknown argument: ${token}`);
    }
  }

  return args;
}

function uniqueStrings(inputs) {
  return Array.from(new Set(inputs.filter(Boolean))).sort();
}

function collectIssueIds(waveEntries) {
  return uniqueStrings(
    waveEntries
      .filter((entry) => entry.event === 'issue-opened' && entry.notes)
      .map((entry) => entry.notes),
  );
}

function collectInvitees(waveEntries) {
  return uniqueStrings(
    waveEntries
      .filter((entry) => entry.event === 'invite-sent')
      .map((entry) => entry.recipient),
  );
}

function buildWaveSummaryPayload(waveSummary, waveEntries, notes, now) {
  return {
    wave: waveSummary.wave,
    generated_at: now.toISOString(),
    invitees: collectInvitees(waveEntries),
    invites: {
      total: waveSummary.recipients?.totalInvites ?? 0,
      open: waveSummary.recipients?.openRecipients ?? 0,
    },
    acknowledgements: waveSummary.acknowledgements ?? 0,
    revocations: waveSummary.revocations ?? 0,
    feedback_submitted: waveSummary.eventCounts?.['feedback-submitted'] ?? 0,
    issue_ids: collectIssueIds(waveEntries),
    response_rate_percent: waveSummary.responseRatePercent ?? null,
    event_counts: waveSummary.eventCounts ?? {},
    last_event_timestamp: waveSummary.lastTimestamp ?? null,
    notes: notes ?? undefined,
  };
}

async function ensureParentDir(filePath) {
  const dir = path.dirname(filePath);
  await mkdir(dir, {recursive: true});
}

async function run() {
  let parsed;
  try {
    parsed = parseArgs(process.argv);
  } catch (error) {
    console.error(`[preview-feedback-wave] ${error.message}`);
    process.exit(1);
  }

  if (parsed.help) {
    process.stdout.write(usage());
    return;
  }
  if (!parsed.wave) {
    console.error('[preview-feedback-wave] --wave is required');
    process.exit(1);
  }

  const logPath = parsed.logPath ?? DEFAULT_LOG_PATH;
  const now = resolveNow();
  const entries = await loadLogFile(logPath);
  const summaries = createSummary(entries, parsed.wave);
  if (summaries.length === 0) {
    console.error(
      `[preview-feedback-wave] no entries found for wave "${parsed.wave}"`,
    );
    process.exit(1);
  }

  const waveSummary = summaries[0];
  const waveEntries = entries.filter((entry) => entry.wave === parsed.wave);
  const normalizedWave =
    normalizeSummary([waveSummary], now)[0] ?? waveSummary;
  if (normalizedWave.lastEventTimestamp && !normalizedWave.lastTimestamp) {
    normalizedWave.lastTimestamp = normalizedWave.lastEventTimestamp;
  }
  const artifactDir = parsed.artifactDir ?? DEFAULT_ARTIFACT_DIR;
  const summaryPath =
    parsed.summaryOut ??
    path.join(artifactDir, `${sanitizeWaveLabel(parsed.wave)}-summary.json`);
  const digestPath =
    parsed.digestOut && parsed.digestOut !== '-'
      ? parsed.digestOut
      : parsed.digestOut === '-'
        ? '-'
        : path.join(artifactDir, `${sanitizeWaveLabel(parsed.wave)}-digest.md`);

  const wavePayload = buildWaveSummaryPayload(
    waveSummary,
    waveEntries,
    parsed.notes,
    now,
  );
  await ensureParentDir(summaryPath);
  await writeFile(`${summaryPath}`, `${JSON.stringify(wavePayload, null, 2)}\n`);
  process.stderr.write(
    `[preview-feedback-wave] wrote wave summary to ${path.relative(process.cwd(), summaryPath)}\n`,
  );

  const inviteWindow = deriveInviteWindow(waveEntries, waveSummary, {
    inviteStart: parsed.inviteStart,
    inviteEnd: parsed.inviteEnd,
  });
  const reportDate =
    parsed.reportDate ?? new Date(now.toISOString()).toISOString().slice(0, 10);

  const logSha = await computeSha256(logPath);
  const summarySha = await computeSha256(summaryPath);

  const digestContent = renderDigest({
    wave: parsed.wave,
    reportDate,
    inviteWindow,
    summary: normalizedWave,
    logInfo: {
      relativePath: path.relative(repoRoot, logPath),
      sha: logSha,
    },
    summaryInfo: {
      relativePath: path.relative(repoRoot, summaryPath),
      sha: summarySha,
    },
    dashboardLink: parsed.dashboardLink,
  });

  if (digestPath === '-') {
    process.stdout.write(digestContent);
  } else {
    await ensureParentDir(digestPath);
    await writeFile(digestPath, digestContent, 'utf8');
    process.stderr.write(
      `[preview-feedback-wave] wrote digest to ${path.relative(process.cwd(), digestPath)}\n`,
    );
  }

  if (parsed.updatePortalData) {
    const portalPayload = buildPortalSummaryPayload(
      entries,
      logPath,
      null,
      now,
    );
    const portalOut = parsed.portalSummaryOut ?? DEFAULT_PORTAL_SUMMARY_PATH;
    await ensureParentDir(portalOut);
    await writeFile(`${portalOut}`, `${JSON.stringify(portalPayload, null, 2)}\n`);
    process.stderr.write(
      `[preview-feedback-wave] refreshed portal data at ${path.relative(process.cwd(), portalOut)}\n`,
    );
  }
}

if (fileURLToPath(import.meta.url) === __filename) {
  run().catch((error) => {
    console.error('[preview-feedback-wave] failed:', error);
    process.exit(1);
  });
}

export {
  buildWaveSummaryPayload,
  collectInvitees,
  collectIssueIds,
  DEFAULT_ARTIFACT_DIR,
  usage,
};
