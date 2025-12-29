#!/usr/bin/env node
import { spawn } from "node:child_process";
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";
import fs from "node:fs/promises";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, "..");
const repoRoot = path.resolve(projectRoot, "..");
const defaultConfigPath = path.join(projectRoot, "scripts", "release_matrix.targets.json");
const defaultOutDir = path.join(repoRoot, "artifacts", "js-sdk-release-matrix");
const defaultShell = process.env.SHELL ?? true;

export function sanitizeLabel(label) {
  if (typeof label !== "string") {
    return "target";
  }
  const normalized = label
    .trim()
    .replace(/\s+/g, "_")
    .replace(/[^A-Za-z0-9._-]/g, "_")
    .replace(/_+/g, "_")
    .replace(/^_+|_+$/g, "");
  return normalized.length > 0 ? normalized : "target";
}

export function buildMarkdownSummary(summary) {
  const generatedAt = summary.generated_at ?? new Date().toISOString();
  const gitRev = summary.git_rev ?? "unknown";
  const matrixName = summary.matrix_name ?? "release_matrix";
  const lines = [
    `# JS SDK Release Matrix (${matrixName})`,
    "",
    `Generated: ${generatedAt}`,
    `Git revision: ${gitRev}`,
    "",
    "| Target | Status | Duration (s) | Node version | Log |",
    "|--------|--------|--------------|--------------|-----|",
  ];
  for (const target of summary.targets ?? []) {
    const durationSeconds =
      typeof target.duration_ms === "number"
        ? (target.duration_ms / 1000).toFixed(1)
        : "n/a";
    lines.push(
      `| ${target.label} | ${target.status} | ${durationSeconds} | ${target.node_version ?? "n/a"} | ${target.log_file ?? "n/a"} |`,
    );
  }
  lines.push("");
  return lines.join("\n");
}

export function buildPrometheusMetrics(summary) {
  const matrixName = summary.matrix_name ?? "release_matrix";
  const gitRev = summary.git_rev ?? "unknown";
  const runTimestamp = toUnixSeconds(summary.generated_at) ?? Math.floor(Date.now() / 1000);
  const targets = Array.isArray(summary.targets) ? summary.targets : [];
  const passed = targets.filter((target) => target.status === "passed").length;
  const failed = targets.length - passed;
  const baseLabels = { matrix_name: matrixName, git_rev: gitRev };
  const lines = [];

  appendMetric(
    lines,
    "js_release_matrix_targets_total",
    "Total JS SDK release matrix targets executed.",
    baseLabels,
    targets.length,
  );
  appendMetric(
    lines,
    "js_release_matrix_targets_passed_total",
    "JS SDK release matrix targets that passed.",
    baseLabels,
    passed,
  );
  appendMetric(
    lines,
    "js_release_matrix_targets_failed_total",
    "JS SDK release matrix targets that failed.",
    baseLabels,
    failed,
  );
  appendMetric(
    lines,
    "js_release_matrix_last_run_timestamp_seconds",
    "Unix timestamp for the latest release matrix run.",
    baseLabels,
    runTimestamp,
  );

  for (const target of targets) {
    const targetLabels = {
      ...baseLabels,
      target: target.label ?? "target",
      status: target.status ?? "unknown",
    };
    if (target.node_version) {
      targetLabels.node_version = target.node_version;
    }
    if (target.log_file) {
      targetLabels.log_file = target.log_file;
    }
    const durationSeconds =
      typeof target.duration_ms === "number" ? target.duration_ms / 1000 : 0;
    appendMetric(
      lines,
      "js_release_matrix_target_duration_seconds",
      "Duration of individual JS SDK release matrix targets.",
      targetLabels,
      durationSeconds,
    );
    appendMetric(
      lines,
      "js_release_matrix_target_exit_code",
      "Exit code emitted by each release matrix target (or -1 when unavailable).",
      targetLabels,
      typeof target.exit_code === "number" ? target.exit_code : -1,
    );
  }

  lines.push("");
  return lines.join("\n");
}

function appendMetric(lines, metric, help, labels, value) {
  lines.push(`# HELP ${metric} ${help}`);
  lines.push(`# TYPE ${metric} gauge`);
  const formattedValue = Number.isFinite(value) ? value : 0;
  lines.push(`${metric}${formatLabels(labels)} ${formattedValue}`);
}

function formatLabels(labels) {
  if (!labels || Object.keys(labels).length === 0) {
    return "";
  }
  const parts = Object.entries(labels).map(
    ([key, rawValue]) => `${key}="${escapeLabelValue(String(rawValue ?? ""))}"`,
  );
  return `{${parts.join(",")}}`;
}

function escapeLabelValue(value) {
  return value
    .replace(/\\/g, "\\\\")
    .replace(/\n/g, "\\n")
    .replace(/"/g, '\\"');
}

function toUnixSeconds(isoTimestamp) {
  if (!isoTimestamp) {
    return undefined;
  }
  const parsed = Date.parse(isoTimestamp);
  if (Number.isNaN(parsed)) {
    return undefined;
  }
  return Math.floor(parsed / 1000);
}

function printUsage() {
  console.log(`Usage: node scripts/release-matrix.mjs [options]\n\n` +
    `Options:\n` +
    `  --config <path>        Path to release_matrix.targets.json (default: ${defaultConfigPath})\n` +
    `  --out-dir <path>       Directory where artifacts are written (default: ${defaultOutDir})\n` +
    `  --matrix-name <name>   Label for this matrix run (default: release_matrix or config value)\n` +
    `  --metrics-out <path>   Optional Prometheus textfile output path (default: <runDir>/matrix.prom)\n` +
    `  --textfile-dir <path>  Mirror metrics to this node_exporter textfile directory\n` +
    `  --help                 Show this message\n`);
}

function parseArgs(argv) {
  const options = {
    configPath: defaultConfigPath,
    outDir: defaultOutDir,
    matrixName: "release_matrix",
    matrixNameFromCli: false,
    metricsOutPath: null,
    textfileDir: null,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--config") {
      if (!argv[i + 1]) {
        throw new Error("--config requires a path");
      }
      options.configPath = path.resolve(argv[i + 1]);
      i += 1;
    } else if (arg.startsWith("--config=")) {
      options.configPath = path.resolve(arg.split("=", 2)[1]);
    } else if (arg === "--out-dir") {
      if (!argv[i + 1]) {
        throw new Error("--out-dir requires a path");
      }
      options.outDir = path.resolve(argv[i + 1]);
      i += 1;
    } else if (arg.startsWith("--out-dir=")) {
      options.outDir = path.resolve(arg.split("=", 2)[1]);
    } else if (arg === "--matrix-name") {
      if (!argv[i + 1]) {
        throw new Error("--matrix-name requires a value");
      }
      options.matrixName = argv[i + 1];
      options.matrixNameFromCli = true;
      i += 1;
    } else if (arg.startsWith("--matrix-name=")) {
      options.matrixName = arg.split("=", 2)[1];
      options.matrixNameFromCli = true;
    } else if (arg === "--metrics-out") {
      if (!argv[i + 1]) {
        throw new Error("--metrics-out requires a path");
      }
      options.metricsOutPath = path.resolve(argv[i + 1]);
      i += 1;
    } else if (arg.startsWith("--metrics-out=")) {
      options.metricsOutPath = path.resolve(arg.split("=", 2)[1]);
    } else if (arg === "--textfile-dir") {
      if (!argv[i + 1]) {
        throw new Error("--textfile-dir requires a path");
      }
      options.textfileDir = path.resolve(argv[i + 1]);
      i += 1;
    } else if (arg.startsWith("--textfile-dir=")) {
      options.textfileDir = path.resolve(arg.split("=", 2)[1]);
    } else if (arg === "--help" || arg === "-h") {
      printUsage();
      process.exit(0);
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }
  return options;
}

async function readConfig(configPath) {
  try {
    const raw = await fs.readFile(configPath, "utf8");
    const parsed = JSON.parse(raw);
    if (Array.isArray(parsed)) {
      return { targets: parsed };
    }
    if (!parsed || !Array.isArray(parsed.targets)) {
      throw new Error("config must be an array or an object with a `targets` array");
    }
    return { targets: parsed.targets, matrixName: parsed.matrixName };
  } catch (error) {
    if (error.code === "ENOENT") {
      throw new Error(
        `config file ${configPath} not found. Copy javascript/iroha_js/scripts/release_matrix.targets.example.json to javascript/iroha_js/scripts/release_matrix.targets.json and edit it.`,
      );
    }
    throw error;
  }
}

function ensureEnvOverrides(envOverrides) {
  if (!envOverrides) {
    return {};
  }
  if (typeof envOverrides !== "object") {
    throw new Error("target env overrides must be an object");
  }
  const out = {};
  for (const [key, value] of Object.entries(envOverrides)) {
    out[key] = value == null ? "" : String(value);
  }
  return out;
}

function coerceTimeout(timeoutSeconds) {
  if (timeoutSeconds == null) {
    return undefined;
  }
  const numeric = Number(timeoutSeconds);
  if (!Number.isFinite(numeric) || numeric <= 0) {
    throw new Error("timeoutSeconds must be a positive number when provided");
  }
  return numeric;
}

function normalizeTargets(rawTargets) {
  return rawTargets.map((entry, index) => {
    if (typeof entry !== "object" || entry === null) {
      throw new Error(`target[${index}] must be an object`);
    }
    if (typeof entry.label !== "string" || !entry.label.trim()) {
      throw new Error(`target[${index}] missing a non-empty string label`);
    }
    if (typeof entry.command !== "string" || !entry.command.trim()) {
      throw new Error(`target[${index}] missing a non-empty string command`);
    }
    const normalized = {
      label: entry.label,
      command: entry.command,
      versionCommand: typeof entry.versionCommand === "string" ? entry.versionCommand : undefined,
      env: ensureEnvOverrides(entry.env),
      timeoutSeconds: coerceTimeout(entry.timeoutSeconds),
      workingDir: typeof entry.workingDir === "string" ? entry.workingDir : ".",
      shell: entry.shell ?? defaultShell,
    };
    return normalized;
  });
}

function runShellCommand(command, options) {
  return new Promise((resolve, reject) => {
    const start = Date.now();
    const child = spawn(command, {
      cwd: options.cwd,
      env: options.env,
      shell: options.shell,
    });
    let stdout = "";
    let stderr = "";
    let timedOut = false;
    const labelPrefix = options.label ? `[${options.label}] ` : "";
    child.stdout.on("data", (chunk) => {
      const text = chunk.toString();
      stdout += text;
      if (options.forwardStdout !== false) {
        process.stdout.write(`${labelPrefix}${text}`);
      }
    });
    child.stderr.on("data", (chunk) => {
      const text = chunk.toString();
      stderr += text;
      if (options.forwardStderr !== false) {
        process.stderr.write(`${labelPrefix}${text}`);
      }
    });
    child.on("error", reject);
    let timeoutId;
    if (options.timeoutSeconds) {
      timeoutId = setTimeout(() => {
        timedOut = true;
        child.kill("SIGKILL");
      }, options.timeoutSeconds * 1000);
    }
    child.on("close", (code, signal) => {
      if (timeoutId) {
        clearTimeout(timeoutId);
      }
      resolve({
        exitCode: code,
        signal,
        stdout,
        stderr,
        durationMs: Date.now() - start,
        timedOut,
      });
    });
  });
}

async function getGitRevision() {
  try {
    const { stdout } = await runShellCommand("git rev-parse HEAD", {
      cwd: repoRoot,
      env: process.env,
      shell: defaultShell,
      label: "git",
      forwardStdout: false,
      forwardStderr: false,
    });
    return stdout.trim();
  } catch (error) {
    console.warn("warning: failed to read git revision", error.message);
    return null;
  }
}

function resolveLogPath(runDir, index, label) {
  const prefix = String(index + 1).padStart(2, "0");
  const sanitized = sanitizeLabel(label);
  return path.join(runDir, `${prefix}_${sanitized}.log`);
}

async function writeLog(logPath, content) {
  await fs.writeFile(logPath, content, "utf8");
}

function relativeToRepo(targetPath) {
  return path.relative(repoRoot, targetPath) || path.basename(targetPath);
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  if (!options.textfileDir && process.env.JS_RELEASE_MATRIX_TEXTFILE_DIR) {
    options.textfileDir = path.resolve(process.env.JS_RELEASE_MATRIX_TEXTFILE_DIR);
  }
  const config = await readConfig(options.configPath);
  if (!options.matrixNameFromCli && typeof config.matrixName === "string") {
    options.matrixName = config.matrixName;
  }
  const targets = normalizeTargets(config.targets);
  const stamp = new Date().toISOString().replace(/[:]/g, "").replace(/\..*/, "");
  const runDirName = `${sanitizeLabel(options.matrixName)}_${stamp}`;
  const runDir = path.join(options.outDir, runDirName);
  await fs.mkdir(runDir, { recursive: true });

  const gitRev = await getGitRevision();
  const summary = {
    generated_at: new Date().toISOString(),
    git_rev: gitRev,
    matrix_name: options.matrixName,
    targets: [],
  };

  let failures = 0;
  for (const [index, target] of targets.entries()) {
    console.log(`\n▶ Running ${target.label}`);
    const resolvedCwd = path.resolve(projectRoot, target.workingDir);
    const env = { ...process.env, ...target.env };
    let nodeVersion;
    if (target.versionCommand) {
      const versionResult = await runShellCommand(target.versionCommand, {
        cwd: resolvedCwd,
        env,
        shell: target.shell,
        label: `${target.label} version`,
        forwardStdout: false,
        forwardStderr: false,
        timeoutSeconds: target.timeoutSeconds,
      });
      if (versionResult.exitCode !== 0) {
        throw new Error(
          `version command for ${target.label} exited with ${versionResult.exitCode}`,
        );
      }
      nodeVersion = versionResult.stdout.trim() || undefined;
    }

    const startIso = new Date().toISOString();
    const result = await runShellCommand(target.command, {
      cwd: resolvedCwd,
      env,
      shell: target.shell,
      label: target.label,
      timeoutSeconds: target.timeoutSeconds,
    });
    const finishIso = new Date().toISOString();
    const status = result.exitCode === 0 && !result.timedOut ? "passed" : "failed";
    if (status === "failed") {
      failures += 1;
    }
    const logPath = resolveLogPath(runDir, index, target.label);
    const logLines = [
      `# ${target.label}`,
      `Command: ${target.command}`,
      target.versionCommand ? `Version command: ${target.versionCommand}` : null,
      `Started: ${startIso}`,
      `Finished: ${finishIso}`,
      `Status: ${status}`,
      `Exit code: ${result.exitCode ?? "n/a"}`,
      `Timed out: ${result.timedOut ? "yes" : "no"}`,
      nodeVersion ? `Node version: ${nodeVersion}` : null,
      "",
      "## STDOUT",
      result.stdout || "(empty)",
      "",
      "## STDERR",
      result.stderr || "(empty)",
      "",
    ].filter(Boolean);
    await writeLog(logPath, logLines.join("\n"));

    summary.targets.push({
      label: target.label,
      status,
      exit_code: result.exitCode,
      duration_ms: result.durationMs,
      started_at: startIso,
      finished_at: finishIso,
      node_version: nodeVersion,
      log_file: relativeToRepo(logPath),
    });
  }

  const summaryPath = path.join(runDir, "matrix.json");
  await fs.writeFile(summaryPath, JSON.stringify(summary, null, 2), "utf8");
  await fs.writeFile(path.join(runDir, "matrix.md"), buildMarkdownSummary(summary), "utf8");
  const metricsPath = options.metricsOutPath ?? path.join(runDir, "matrix.prom");
  await fs.mkdir(path.dirname(metricsPath), { recursive: true });
  await fs.writeFile(metricsPath, buildPrometheusMetrics(summary), "utf8");
  const textfileTarget = options.textfileDir;
  if (textfileTarget) {
    await mirrorMetricsFile(metricsPath, textfileTarget);
  }

  console.log(`\nArtifacts written to ${relativeToRepo(runDir)}`);
  if (failures > 0) {
    console.error(`${failures} target(s) failed`);
    process.exitCode = 1;
  }
}

async function mirrorMetricsFile(sourcePath, textfileDir) {
  try {
    await fs.mkdir(textfileDir, { recursive: true });
    const targetPath = path.join(textfileDir, "js_release_matrix.prom");
    const tempPath = `${targetPath}.${process.pid}.${Date.now()}`;
    await fs.copyFile(sourcePath, tempPath);
    await fs.rename(tempPath, targetPath);
    console.log(`[release-matrix] mirrored metrics to ${targetPath}`);
  } catch (error) {
    console.warn(
      `[release-matrix] failed to mirror metrics to ${textfileDir}: ${error.message}`,
    );
  }
}

function invokedDirectly() {
  if (!process.argv[1]) {
    return false;
  }
  try {
    const invokedPath = pathToFileURL(path.resolve(process.argv[1])).href;
    return invokedPath === import.meta.url;
  } catch {
    return false;
  }
}

if (invokedDirectly()) {
  main().catch((error) => {
    console.error("release matrix failed", error);
    process.exit(1);
  });
}
