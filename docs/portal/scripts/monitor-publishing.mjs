#!/usr/bin/env node
// SPDX-License-Identifier: Apache-2.0

import {readFile, writeFile, mkdir} from 'node:fs/promises';
import {execFile} from 'node:child_process';
import {Resolver} from 'node:dns/promises';
import {createHash} from 'node:crypto';
import {parseArgs} from 'node:util';
import {fileURLToPath} from 'node:url';
import path from 'node:path';

import {
  probePath,
  validateSecurityMeta,
} from './portal-probe.mjs';
import {
  runProbe as runTryItProbe,
  formatPrometheusLabels,
  verifyMetricsEndpoint,
} from './tryit-proxy-probe.mjs';

const defaultDnsResolver = new Resolver();
const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const DEFAULT_BINDING_TIMEOUT_MS = 30_000;

/**
 * Load the monitor configuration from a JSON file.
 *
 * @param {string | undefined} configPath
 * @returns {Promise<object>}
 */
export async function loadMonitorConfig(configPath) {
  if (!configPath) {
    return {};
  }
  const trimmed = configPath.trim();
  if (trimmed === '') {
    return {};
  }
  const raw = await readFile(trimmed, {encoding: 'utf8'});
  try {
    return JSON.parse(raw);
  } catch (error) {
    throw new Error(`failed to parse monitor config ${trimmed}: ${error.message}`);
  }
}

function hasSecurityExpectations(expected) {
  const normalise = (value) => (value ?? '').replace(/\s+/g, ' ').trim();
  return (
    normalise(expected?.csp) !== '' ||
    normalise(expected?.permissionsPolicy) !== '' ||
    normalise(expected?.referrerPolicy) !== ''
  );
}

/**
 * Monitor portal routes and security metadata.
 *
 * @param {object | undefined} portalConfig
 * @param {object} deps
 * @param {Function} deps.probePathImpl
 * @param {Function} deps.validateSecurityImpl
 * @returns {Promise<object | null>}
 */
export async function monitorPortal(
  portalConfig,
  {
    probePathImpl = probePath,
    validateSecurityImpl = validateSecurityMeta,
  } = {},
) {
  if (!portalConfig || !portalConfig.baseUrl) {
    return {
      target: 'portal',
      skipped: true,
      reason: 'baseUrl not provided',
    };
  }

  const baseUrl = portalConfig.baseUrl;
  const allowInsecureHttp = portalConfig.allowInsecureHttp === true;
  let parsedBaseUrl;
  try {
    parsedBaseUrl = new URL(baseUrl);
  } catch (error) {
    return {
      target: 'portal',
      baseUrl,
      skipped: false,
      ok: false,
      entries: [
        {
          path: '/',
          url: baseUrl,
          failures: [`invalid baseUrl: ${error.message}`],
          ok: false,
        },
      ],
    };
  }

  if (parsedBaseUrl.protocol !== 'https:' && !allowInsecureHttp) {
    return {
      target: 'portal',
      baseUrl,
      skipped: false,
      ok: false,
      entries: [
        {
          path: '/',
          url: parsedBaseUrl.toString(),
          failures: [
            "insecure baseUrl rejected; use HTTPS or set allowInsecureHttp=true for local previews",
          ],
          ok: false,
        },
      ],
    };
  }

  const expectRelease = portalConfig.expectRelease ?? '';
  const paths = Array.isArray(portalConfig.paths) && portalConfig.paths.length > 0
    ? portalConfig.paths
    : ['/'];
  const checkSecurity =
    portalConfig.checkSecurity !== undefined ? Boolean(portalConfig.checkSecurity) : true;
  const expectedSecurity = portalConfig.expectedSecurity ?? {};
  const enforceSecurity = checkSecurity && hasSecurityExpectations(expectedSecurity);
  const entries = [];
  let ok = true;

  for (const segment of paths) {
    const pathLiteral = segment || '/';
    let result;
    try {
      result = await probePathImpl(baseUrl, pathLiteral);
    } catch (error) {
      ok = false;
      entries.push({
        path: pathLiteral,
        url: new URL(pathLiteral, baseUrl).toString(),
        ok: false,
        error: error.message ?? String(error),
      });
      continue;
    }
    const failures = [];
    if (!result.ok) {
      failures.push(`HTTP ${result.status}`);
    }
    if (expectRelease && result.release !== expectRelease) {
      failures.push(
        `release mismatch (expected ${expectRelease}, saw ${result.release || 'n/a'})`,
      );
    }
    if (enforceSecurity) {
      const issues = validateSecurityImpl(result.security, expectedSecurity);
      if (issues.length > 0) {
        failures.push(`security mismatch: ${issues.join('; ')}`);
      }
    }
    const entryOk = failures.length === 0;
    ok = ok && entryOk;
    entries.push({
      path: pathLiteral,
      url: result.url,
      status: result.status,
      durationMs: result.durationMs,
      release: result.release,
      failures,
      ok: entryOk,
    });
  }

  return {
    target: 'portal',
    baseUrl,
    expectRelease,
    checkSecurity: enforceSecurity,
    skipped: false,
    ok,
    entries,
  };
}

/**
 * Monitor the Try it proxy health.
 *
 * @param {object | undefined} tryItConfig
 * @param {object} deps
 * @param {Function} deps.runProbeImpl
 * @param {Function} deps.verifyMetricsImpl
 * @returns {Promise<object>}
 */
export async function monitorTryIt(
  tryItConfig,
  {
    runProbeImpl = runTryItProbe,
    verifyMetricsImpl = verifyMetricsEndpoint,
  } = {},
) {
  if (!tryItConfig || !tryItConfig.proxyUrl) {
    return {
      target: 'tryit-proxy',
      skipped: true,
      reason: 'proxyUrl not provided',
    };
  }

  const proxyUrl = tryItConfig.proxyUrl.replace(/\/$/, '');
  const samplePath = tryItConfig.samplePath ?? '';
  const method = (tryItConfig.method ?? 'GET').toUpperCase();
  const timeoutMs =
    typeof tryItConfig.timeoutMs === 'number' ? tryItConfig.timeoutMs : 5_000;
  const token = tryItConfig.token ?? '';
  const metricsUrl = tryItConfig.metricsUrl ?? '';
  const summary = {
    target: 'tryit-proxy',
    proxyUrl,
    samplePath,
    method,
    timeoutMs,
    skipped: false,
    ok: true,
    durationMs: 0,
  };

  const start = Date.now();
  try {
    await runProbeImpl({
      proxyUrl,
      samplePath,
      method,
      timeoutMs,
      token,
      fetchImpl: globalThis.fetch,
    });
    if (metricsUrl) {
      await verifyMetricsImpl({
        metricsUrl,
        timeoutMs,
        fetchImpl: globalThis.fetch,
      });
    }
  } catch (error) {
    summary.ok = false;
    summary.error = error.message ?? String(error);
  } finally {
    summary.durationMs = Date.now() - start;
  }

  return summary;
}

function normaliseBindingConfigs(bindingConfig) {
  if (!bindingConfig) {
    return [];
  }
  if (Array.isArray(bindingConfig)) {
    return bindingConfig;
  }
  return [bindingConfig];
}

function resolveLocalPath(value) {
  if (typeof value !== 'string') {
    return '';
  }
  const trimmed = value.trim();
  if (trimmed === '') {
    return '';
  }
  if (path.isAbsolute(trimmed)) {
    return trimmed;
  }
  return path.resolve(process.cwd(), trimmed);
}

function quoteArg(value) {
  if (value === '') {
    return "''";
  }
  if (/[\s"'`$\\]/.test(value)) {
    return `"${value.replace(/(["\\$`])/g, '\\$1')}"`;
  }
  return value;
}

function formatCommand(binary, args) {
  return [binary, ...args.map(quoteArg)].join(' ');
}

async function execCommand(
  binary,
  args,
  {cwd, timeoutMs = DEFAULT_BINDING_TIMEOUT_MS, execFileImpl = execFile} = {},
) {
  return new Promise((resolve, reject) => {
    execFileImpl(
      binary,
      args,
      {
        cwd,
        timeout: timeoutMs,
        maxBuffer: 10 * 1024 * 1024,
      },
      (error, stdout, stderr) => {
        if (error) {
          const message = error.killed
            ? `command timed out after ${timeoutMs} ms`
            : error.message;
          const wrapped = new Error(message);
          wrapped.code = error.code;
          wrapped.signal = error.signal;
          wrapped.stdout = stdout;
          wrapped.stderr = stderr;
          reject(wrapped);
          return;
        }
        resolve({stdout, stderr});
      },
    );
  });
}

/**
 * Build the soradns binding verification command.
 *
 * @param {object} options
 * @returns {{command: string, args: string[], bindingPath: string, manifestJson: string | null}}
 */
export function buildBindingCommand({
  bindingPath,
  alias,
  contentCid,
  hostname,
  proofStatus,
  manifestJson,
} = {}) {
  const resolvedBinding = resolveLocalPath(bindingPath);
  if (!resolvedBinding) {
    throw new Error('binding path is required');
  }
  const resolvedManifest = resolveLocalPath(manifestJson);
  const args = ['xtask', 'soradns-verify-binding', '--binding', resolvedBinding];
  if (alias) {
    args.push('--alias', alias);
  }
  if (contentCid) {
    args.push('--content-cid', contentCid);
  }
  if (hostname) {
    args.push('--hostname', hostname);
  }
  if (proofStatus) {
    args.push('--proof-status', proofStatus);
  }
  if (resolvedManifest) {
    args.push('--manifest-json', resolvedManifest);
  }
  return {
    command: formatCommand('cargo', args),
    args,
    bindingPath: resolvedBinding,
    manifestJson: resolvedManifest || null,
  };
}

/**
 * Verify a gateway binding using cargo xtask.
 *
 * @param {object} options
 * @returns {Promise<{command: string, stdout: string, stderr: string}>}
 */
export async function verifyBindingViaXtask({
  bindingPath,
  alias,
  contentCid,
  hostname,
  proofStatus,
  manifestJson,
  timeoutMs = DEFAULT_BINDING_TIMEOUT_MS,
  execFileImpl = execFile,
} = {}) {
  const {command, args} = buildBindingCommand({
    bindingPath,
    alias,
    contentCid,
    hostname,
    proofStatus,
    manifestJson,
  });
  try {
    const result = await execCommand('cargo', args, {
      cwd: REPO_ROOT,
      timeoutMs,
      execFileImpl,
    });
    return {
      command,
      stdout: result.stdout?.trim() ?? '',
      stderr: result.stderr?.trim() ?? '',
    };
  } catch (error) {
    error.command = command;
    throw error;
  }
}

function normaliseDnsRecordsConfig(dnsConfig) {
  if (!dnsConfig) {
    return [];
  }
  if (Array.isArray(dnsConfig)) {
    return dnsConfig;
  }
  return [dnsConfig];
}

function flattenTxtRecord(value) {
  if (Array.isArray(value)) {
    return value.map((entry) => flattenTxtRecord(entry)).join('');
  }
  if (value === null || value === undefined) {
    return '';
  }
  return String(value);
}

function normaliseDnsAnswer(recordType, value) {
  if (value === null || value === undefined) {
    return '';
  }
  if (recordType === 'TXT') {
    return flattenTxtRecord(value).trim();
  }
  if (Array.isArray(value)) {
    return value.map((entry) => normaliseDnsAnswer(recordType, entry)).join(' ');
  }
  if (typeof value === 'object') {
    return JSON.stringify(value);
  }
  return String(value);
}

function shouldLowercaseRecord(recordType) {
  return recordType === 'CNAME' || recordType === 'NS' || recordType === 'PTR';
}

function cleanDnsValue(recordType, value) {
  const trimmed = value.trim();
  if (trimmed === '') {
    return '';
  }
  const withoutTrailingDot = trimmed.endsWith('.')
    ? trimmed.slice(0, trimmed.length - 1)
    : trimmed;
  const normalisedWhitespace = withoutTrailingDot.replace(/\s+/g, ' ');
  if (shouldLowercaseRecord(recordType)) {
    return normalisedWhitespace.toLowerCase();
  }
  return normalisedWhitespace;
}

function normaliseDnsAnswers(recordType, answers) {
  if (!Array.isArray(answers)) {
    return [];
  }
  const flattened = answers
    .map((value) => normaliseDnsAnswer(recordType, value))
    .map((value) => cleanDnsValue(recordType, value))
    .filter((value) => value !== '');
  return flattened;
}

function normaliseExpectedDnsValues(recordType, values) {
  if (!values && values !== '') {
    return [];
  }
  const payload = Array.isArray(values) ? values : [values];
  return payload
    .map((value) =>
      typeof value === 'string' ? value : value == null ? '' : String(value),
    )
    .map((value) => cleanDnsValue(recordType, value))
    .filter((value) => value !== '');
}

/**
 * Verify that the published binding advertises the expected manifest.
 *
 * @param {object | Array<object> | undefined} bindingConfig
 * @param {object} deps
 * @param {Function} deps.verifyBindingImpl
 * @returns {Promise<object>}
 */
export async function monitorBinding(
  bindingConfig,
  {
    verifyBindingImpl = verifyBindingViaXtask,
  } = {},
) {
  const payloads = normaliseBindingConfigs(bindingConfig);
  if (payloads.length === 0) {
    return {
      target: 'sorafs-binding',
      skipped: true,
      reason: 'binding path not provided',
    };
  }

  const entries = [];
  let ok = true;

  for (let index = 0; index < payloads.length; index += 1) {
    const item = payloads[index] ?? {};
    const bindingPath = resolveLocalPath(item.bindingPath);
    const manifestJson = resolveLocalPath(item.manifestJson);
    const label =
      item.label ??
      item.alias ??
      item.hostname ??
      (bindingPath ? path.basename(bindingPath) : null) ??
      `binding-${index + 1}`;

    if (!bindingPath) {
      ok = false;
      entries.push({
        label,
        ok: false,
        error: 'binding path not provided',
      });
      continue;
    }

    const commandInfo = buildBindingCommand({
      bindingPath,
      alias: item.alias,
      contentCid: item.contentCid,
      hostname: item.hostname,
      proofStatus: item.proofStatus,
      manifestJson,
    });

    try {
      const summary = await verifyBindingImpl({
        bindingPath,
        alias: item.alias,
        contentCid: item.contentCid,
        hostname: item.hostname,
        proofStatus: item.proofStatus,
        manifestJson,
        timeoutMs: item.timeoutMs ?? DEFAULT_BINDING_TIMEOUT_MS,
      });
      entries.push({
        label,
        bindingPath,
        ok: true,
        alias: item.alias ?? null,
        contentCid: item.contentCid ?? null,
        hostname: item.hostname ?? null,
        proofStatus: item.proofStatus ?? null,
        manifestJson: manifestJson || null,
        command: summary?.command ?? commandInfo.command,
        stdout: summary?.stdout ?? null,
        stderr: summary?.stderr ?? null,
        summary,
      });
    } catch (error) {
      ok = false;
      entries.push({
        label,
        bindingPath,
        ok: false,
        error: error.message ?? String(error),
        command: error.command ?? commandInfo.command,
        stdout: error.stdout?.trim() ?? null,
        stderr: error.stderr?.trim() ?? null,
      });
    }
  }

  return {
    target: 'sorafs-binding',
    skipped: false,
    ok,
    entries,
  };
}

const DEFAULT_DNS_TIMEOUT_MS = 5000;

async function defaultResolveRecord(hostname, recordType) {
  return defaultDnsResolver.resolve(hostname, recordType);
}

function normaliseDnsHostname(entry) {
  const host =
    entry.hostname || entry.fqdn || entry.host || entry.name || entry.domain;
  if (typeof host !== 'string') {
    return '';
  }
  return host.trim();
}

function normaliseDnsRecordType(entry) {
  const raw = typeof entry.recordType === 'string' ? entry.recordType : entry.type;
  if (typeof raw !== 'string' || raw.trim() === '') {
    return 'A';
  }
  return raw.trim().toUpperCase();
}

function normaliseDnsTimeout(entry) {
  const value = entry.timeoutMs ?? entry.timeout;
  if (typeof value === 'number' && Number.isFinite(value) && value > 0) {
    return value;
  }
  return DEFAULT_DNS_TIMEOUT_MS;
}

function buildDnsFailure(message) {
  return message || 'dns validation failed';
}

function summaryFromDnsEntry(entry, answers, failures, extra = {}) {
  const summary = {
    label: entry.label || entry.hostname || entry.host || entry.name || 'dns-record',
    hostname: entry.hostname,
    recordType: entry.recordType,
    answers,
    ok: failures.length === 0,
    failures,
  };
  return {...summary, ...extra};
}

export async function monitorDnsRecords(
  dnsConfig,
  {resolveRecordImpl = defaultResolveRecord} = {},
) {
  const entries = normaliseDnsRecordsConfig(dnsConfig);
  if (entries.length === 0) {
    return {
      target: 'soradns',
      skipped: true,
      reason: 'dns records not provided',
    };
  }

  const summaries = [];
  let ok = true;

  for (const entry of entries) {
    const hostname = normaliseDnsHostname(entry);
    const recordType = normaliseDnsRecordType(entry);
    const timeoutMs = normaliseDnsTimeout(entry);
    const requireAnswers = entry.requireAnswers !== false;
    const expectedExact = normaliseExpectedDnsValues(
      recordType,
      entry.expectedRecords ?? entry.expect ?? entry.records,
    );
    const expectedIncludes = normaliseExpectedDnsValues(
      recordType,
      entry.expectedIncludes ?? entry.includes ?? [],
    );

    if (!hostname) {
      summaries.push(
        summaryFromDnsEntry(
          {label: entry.label || 'dns-record', hostname, recordType},
          [],
          [buildDnsFailure('hostname is required')],
        ),
      );
      ok = false;
      continue;
    }

    let answers = [];
    const failures = [];

    let timeoutId;
    try {
      const timeoutPromise = new Promise((_, reject) => {
        timeoutId = setTimeout(
          () => reject(new Error(`lookup timed out after ${timeoutMs} ms`)),
          timeoutMs,
        );
      });
      const result = await Promise.race([
        resolveRecordImpl(hostname, recordType),
        timeoutPromise,
      ]);
      answers = normaliseDnsAnswers(recordType, result);
    } catch (error) {
      failures.push(buildDnsFailure(error?.message || String(error)));
    } finally {
      if (timeoutId) {
        clearTimeout(timeoutId);
      }
    }

    if (failures.length === 0) {
      if (requireAnswers && answers.length === 0) {
        failures.push('no DNS answers returned');
      }
      if (expectedExact.length > 0) {
        const actualSorted = [...answers].sort();
        const expectedSorted = [...expectedExact].sort();
        const mismatch =
          actualSorted.length !== expectedSorted.length ||
          actualSorted.some((value, index) => value !== expectedSorted[index]);
        if (mismatch) {
          failures.push(
            `record set mismatch: expected [${expectedSorted.join(', ')}], received [${actualSorted.join(', ')}]`,
          );
        }
      }
      if (expectedIncludes.length > 0) {
        for (const expected of expectedIncludes) {
          if (!answers.includes(expected)) {
            failures.push(`missing required record: ${expected}`);
          }
        }
      }
    }

    ok = ok && failures.length === 0;
    summaries.push(
      summaryFromDnsEntry(
        {label: entry.label, hostname, recordType},
        answers,
        failures,
        {
          expectedRecords: expectedExact.length > 0 ? expectedExact : undefined,
          expectedIncludes:
            expectedIncludes.length > 0 ? expectedIncludes : undefined,
          timeoutMs,
        },
      ),
    );
  }

  return {
    target: 'soradns',
    skipped: false,
    ok,
    entries: summaries,
  };
}

/**
 * Run all configured monitors and return a consolidated summary.
 *
 * @param {object} config
 * @param {object} deps
 * @returns {Promise<object>}
 */
export async function runMonitor(config = {}, deps = {}) {
  const portalResult = await monitorPortal(config.portal, deps.portal);
  const tryItResult = await monitorTryIt(config.tryIt, deps.tryIt);
  const bindingResult = await monitorBinding(
    config.binding ?? config.bindings,
    deps.binding,
  );
  const dnsResult = await monitorDnsRecords(config.dns ?? config.soradns, deps.dns);

  const sections = [portalResult, tryItResult, bindingResult, dnsResult].filter(
    Boolean,
  );
  const failures = sections.filter(
    (section) => section.skipped !== true && section.ok === false,
  );

  return {
    timestamp: new Date().toISOString(),
    portal: portalResult,
    tryIt: tryItResult,
    binding: bindingResult,
    dns: dnsResult,
    ok: failures.length === 0,
  };
}

async function writeSummary(jsonPath, summary) {
  if (!jsonPath) {
    return;
  }
  const dir = path.dirname(jsonPath);
  await mkdir(dir, {recursive: true});
  await writeFile(jsonPath, JSON.stringify(summary, null, 2) + '\n', {
    encoding: 'utf8',
  });
}

/**
 * Persist per-section monitor output together with deterministic digests.
 *
 * @param {string | undefined} evidenceDir
 * @param {object} summary
 */
export async function writeEvidenceBundle(evidenceDir, summary) {
  if (!evidenceDir) {
    return;
  }
  const targetDir = evidenceDir.trim();
  if (targetDir === '') {
    return;
  }
  await mkdir(targetDir, {recursive: true});
  const files = [
    ['summary.json', summary],
    ['portal.json', summary.portal],
    ['tryit.json', summary.tryIt],
    ['binding.json', summary.binding],
    ['dns.json', summary.dns],
  ].filter(([, data]) => data !== undefined);
  if (files.length === 0) {
    return;
  }
  const checksumEntries = [];
  for (const [filename, data] of files) {
    const payload = JSON.stringify(data, null, 2) + '\n';
    const filepath = path.join(targetDir, filename);
    await writeFile(filepath, payload, {encoding: 'utf8'});
    const digest = createHash('sha256').update(payload).digest('hex');
    checksumEntries.push(`${digest}  ${filename}`);
  }
  const checksumPath = path.join(targetDir, 'checksums.sha256');
  await writeFile(checksumPath, checksumEntries.join('\n') + '\n', {encoding: 'utf8'});
}

function gauge(value) {
  return value ? 1 : 0;
}

function renderProbeMetrics(section, baseLabels, lines) {
  if (!section || section.skipped) {
    return;
  }
  const labels = {...baseLabels, target: section.target};
  lines.push(
    `docs_monitor_up${formatPrometheusLabels(labels)} ${gauge(section.ok)}`,
  );
  if (section.durationMs !== undefined) {
    lines.push(
      `docs_monitor_probe_duration_seconds${formatPrometheusLabels(labels)} ${(Math.max(section.durationMs, 0) / 1000).toFixed(3)}`,
    );
  }
  if (Array.isArray(section.entries)) {
    for (const entry of section.entries) {
      const entryLabels = {...labels};
      if (entry.path) {
        entryLabels.path = entry.path;
      } else if (entry.url) {
        entryLabels.url = entry.url;
      }
      if (entry.label) {
        entryLabels.label = entry.label;
      }
      lines.push(
        `docs_monitor_entry_ok${formatPrometheusLabels(entryLabels)} ${gauge(entry.ok)}`,
      );
      if (entry.durationMs !== undefined) {
        lines.push(
          `docs_monitor_entry_duration_seconds${formatPrometheusLabels(entryLabels)} ${(Math.max(entry.durationMs, 0) / 1000).toFixed(3)}`,
        );
      }
    }
  }
}

function renderBindingMetrics(section, baseLabels, lines) {
  if (!section || section.skipped || !Array.isArray(section.entries)) {
    return;
  }
  for (const entry of section.entries) {
    const labels = {...baseLabels, target: section.target ?? 'binding'};
    if (entry.label) {
      labels.label = entry.label;
    }
    if (entry.alias) {
      labels.alias = entry.alias;
    }
    if (entry.hostname) {
      labels.hostname = entry.hostname;
    }
    lines.push(
      `docs_monitor_entry_ok${formatPrometheusLabels(labels)} ${gauge(entry.ok)}`,
    );
  }
}

function renderDnsMetrics(section, baseLabels, lines) {
  if (!section || section.skipped || !Array.isArray(section.entries)) {
    return;
  }
  for (const entry of section.entries) {
    const labels = {...baseLabels, target: section.target ?? 'dns'};
    if (entry.hostname) {
      labels.hostname = entry.hostname;
    }
    if (entry.recordType) {
      labels.record_type = entry.recordType;
    }
    lines.push(
      `docs_monitor_entry_ok${formatPrometheusLabels(labels)} ${gauge(entry.ok)}`,
    );
  }
}

/**
 * Render Prometheus text-format metrics for the monitor summary.
 *
 * @param {object} summary
 * @param {object} [options]
 * @param {object} [options.labels]
 * @returns {string}
 */
export function renderPrometheusMetrics(summary, {labels = {}} = {}) {
  const lines = [];
  lines.push(
    '# HELP docs_monitor_up 1 indicates the monitor section is healthy',
  );
  lines.push('# TYPE docs_monitor_up gauge');
  lines.push(
    '# HELP docs_monitor_probe_duration_seconds Duration of individual probes',
  );
  lines.push('# TYPE docs_monitor_probe_duration_seconds gauge');
  lines.push('# HELP docs_monitor_entry_ok Entry-level success indicator');
  lines.push('# TYPE docs_monitor_entry_ok gauge');
  lines.push(
    '# HELP docs_monitor_entry_duration_seconds Per-entry probe duration',
  );
  lines.push('# TYPE docs_monitor_entry_duration_seconds gauge');

  renderProbeMetrics(summary.portal, labels, lines);
  renderProbeMetrics(summary.tryIt, labels, lines);
  renderBindingMetrics(summary.binding, labels, lines);
  renderDnsMetrics(summary.dns, labels, lines);

  return lines.join('\n') + '\n';
}

async function writePrometheusMetrics(promPath, summary, options) {
  if (!promPath) {
    return;
  }
  const payload = renderPrometheusMetrics(summary, options);
  const dir = path.dirname(promPath);
  await mkdir(dir, {recursive: true});
  await writeFile(promPath, payload, {encoding: 'utf8'});
}

function reportSection(section) {
  if (!section) {
    return;
  }
  if (section.skipped) {
    console.warn(`[monitor-publishing] ${section.target} check skipped: ${section.reason}`);
    return;
  }
  if (section.ok) {
    console.log(`[monitor-publishing] ${section.target} ok`);
    return;
  }
  console.error(`[monitor-publishing] ${section.target} failed`);
  if (section.entries) {
    for (const entry of section.entries) {
      if (entry.ok) {
        continue;
      }
      console.error(
        `  target=${entry.path ?? entry.label ?? entry.bindingPath ?? entry.url ?? 'n/a'} ${entry.failures?.join('; ') ?? entry.error ?? 'unknown error'}`,
      );
    }
  } else if (section.error) {
    console.error(`  ${section.error}`);
  }
}

async function main() {
  const {values} = parseArgs({
    options: {
      config: {type: 'string'},
      'json-out': {type: 'string'},
      'evidence-dir': {type: 'string'},
      'prom-out': {type: 'string'},
      'prom-job': {type: 'string'},
    },
  });
  const configPath = values.config ?? process.env.DOCS_MONITOR_CONFIG ?? '';
  const config = await loadMonitorConfig(configPath);
  const summary = await runMonitor(config);
  reportSection(summary.portal);
  reportSection(summary.tryIt);
  reportSection(summary.binding);
  reportSection(summary.dns);
  if (values['json-out']) {
    await writeSummary(values['json-out'], summary);
    console.log(`[monitor-publishing] wrote summary to ${values['json-out']}`);
  }
  if (values['evidence-dir']) {
    await writeEvidenceBundle(values['evidence-dir'], summary);
    console.log(`[monitor-publishing] wrote evidence bundle to ${values['evidence-dir']}`);
  }
  if (values['prom-out']) {
    await writePrometheusMetrics(values['prom-out'], summary, {
      labels: {
        job: values['prom-job'] ?? process.env.DOCS_MONITOR_JOB ?? 'docs-monitor',
      },
    });
    console.log(`[monitor-publishing] wrote Prometheus metrics to ${values['prom-out']}`);
  }
  if (!summary.ok) {
    process.exitCode = 1;
  }
}

const isCli =
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (isCli) {
  main().catch((error) => {
    console.error('[monitor-publishing] fatal error', error);
    process.exitCode = 1;
  });
}
