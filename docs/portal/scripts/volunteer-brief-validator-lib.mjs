import path from 'node:path';

export function buildPortalSummary(cliReport = {}, options = {}) {
  const now = options.now ?? new Date();
  const repoRoot = options.repoRoot ?? process.cwd();
  const entries = Array.isArray(cliReport.entries) ? cliReport.entries : [];
  const normalizedEntries = entries.map((entry) => normalizeEntry(entry, repoRoot));

  const proposalMap = new Map();
  for (const entry of normalizedEntries) {
    const key = entry.proposalId ?? 'unknown';
    let bucket = proposalMap.get(key);
    if (!bucket) {
      bucket = {
        proposalId: key,
        totalBriefs: 0,
        blocked: 0,
        warning: 0,
        support: 0,
        oppose: 0,
        context: 0,
        other: 0,
        entries: []
      };
      proposalMap.set(key, bucket);
    }
    bucket.totalBriefs += 1;
    if (entry.status === 'blocked') {
      bucket.blocked += 1;
    } else if (entry.status === 'warning') {
      bucket.warning += 1;
    }
    if (entry.stance === 'support') {
      bucket.support += 1;
    } else if (entry.stance === 'oppose') {
      bucket.oppose += 1;
    } else if (entry.stance === 'context') {
      bucket.context += 1;
    } else {
      bucket.other += 1;
    }
    bucket.entries.push(entry);
  }

  const proposals = Array.from(proposalMap.values())
    .map((bucket) => ({
      ...bucket,
      entries: bucket.entries.sort(compareEntries)
    }))
    .sort((a, b) => a.proposalId.localeCompare(b.proposalId));

  const fallbackTotal = normalizedEntries.length;
  const failingFallback = normalizedEntries.filter((entry) => entry.status === 'blocked').length;
  const warningFallback = normalizedEntries.filter((entry) => entry.status === 'warning').length;
  const totalErrorsFallback = normalizedEntries.reduce((sum, entry) => sum + entry.errorCount, 0);
  const totalWarningsFallback = normalizedEntries.reduce(
    (sum, entry) => sum + entry.warningCount,
    0
  );

  const cliGeneratedAtIso =
    typeof cliReport.generated_at_unix_ms === 'number'
      ? new Date(cliReport.generated_at_unix_ms).toISOString()
      : null;

  return {
    version: 1,
    generatedAtIso: now.toISOString(),
    cliGeneratedAtIso,
    summary: {
      inputs: Array.isArray(cliReport.inputs) ? cliReport.inputs : [],
      totalBriefs:
        typeof cliReport.total_entries === 'number' ? cliReport.total_entries : fallbackTotal,
      failingBriefs:
        typeof cliReport.entries_with_errors === 'number'
          ? cliReport.entries_with_errors
          : failingFallback,
      warningBriefs: warningFallback,
      totalErrors:
        typeof cliReport.total_errors === 'number' ? cliReport.total_errors : totalErrorsFallback,
      totalWarnings:
        typeof cliReport.total_warnings === 'number'
          ? cliReport.total_warnings
          : totalWarningsFallback
    },
    proposals,
    entries: normalizedEntries,
    cli: cliReport
  };
}

function normalizeEntry(entry, repoRoot) {
  const metadata = isPlainObject(entry?.metadata) ? entry.metadata : {};
  const errors = Array.isArray(entry?.errors) ? entry.errors : [];
  const warnings = Array.isArray(entry?.warnings) ? entry.warnings : [];
  const status = errors.length > 0 ? 'blocked' : warnings.length > 0 ? 'warning' : 'pass';
  const stance = (cleanString(metadata.stance) ?? 'unknown').toLowerCase();

  return {
    proposalId: cleanString(metadata.proposal_id) ?? 'unknown',
    briefId: cleanString(metadata.brief_id),
    stance,
    language: cleanString(metadata.language),
    submittedAt: cleanString(metadata.submitted_at),
    moderationOffTopic:
      typeof metadata.moderation_off_topic === 'boolean'
        ? metadata.moderation_off_topic
        : null,
    errors,
    warnings,
    errorCount: errors.length,
    warningCount: warnings.length,
    status,
    inputPath: normalizePath(entry?.input_path, repoRoot)
  };
}

function normalizePath(inputPath, repoRoot) {
  if (typeof inputPath !== 'string' || inputPath.length === 0) {
    return null;
  }
  const sanitizedRoot = repoRoot ?? process.cwd();
  const normalized = path.isAbsolute(inputPath)
    ? path.relative(sanitizedRoot, inputPath)
    : inputPath;
  const cleaned = normalized.length === 0 ? '.' : normalized;
  return cleaned.split(path.sep).join('/');
}

function compareEntries(a, b) {
  const left = a.briefId ?? a.inputPath ?? '';
  const right = b.briefId ?? b.inputPath ?? '';
  return left.localeCompare(right);
}

function cleanString(value) {
  if (typeof value !== 'string') {
    return null;
  }
  const trimmed = value.trim();
  return trimmed.length === 0 ? null : trimmed;
}

function isPlainObject(value) {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
