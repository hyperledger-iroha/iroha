import test from "node:test";
import assert from "node:assert/strict";

import {
  buildMarkdownSummary,
  buildPrometheusMetrics,
  sanitizeLabel,
} from "../scripts/release-matrix.mjs";

test("sanitizeLabel normalizes whitespace and punctuation", () => {
  assert.equal(sanitizeLabel(" Node 20 / Linux "), "Node_20_Linux");
  assert.equal(sanitizeLabel("node-20"), "node-20");
});

test("sanitizeLabel falls back to target", () => {
  assert.equal(sanitizeLabel("   \t"), "target");
  assert.equal(sanitizeLabel("🚀"), "target");
});

test("buildMarkdownSummary renders release table", () => {
  const summary = {
    generated_at: "2026-01-27T00:00:00Z",
    git_rev: "abc123",
    matrix_name: "demo",
    targets: [
      {
        label: "node20",
        status: "passed",
        duration_ms: 1200,
        node_version: "v20.11.0",
        log_file: "artifacts/matrix/node20.log",
      },
      {
        label: "node18",
        status: "failed",
        duration_ms: undefined,
        node_version: undefined,
        log_file: "artifacts/matrix/node18.log",
      },
    ],
  };

  const markdown = buildMarkdownSummary(summary);
  assert.ok(markdown.includes("JS SDK Release Matrix (demo)"));
  assert.ok(
    markdown.includes("| node20 | passed | 1.2 | v20.11.0 | artifacts/matrix/node20.log |"),
  );
  assert.ok(markdown.includes("| node18 | failed | n/a | n/a | artifacts/matrix/node18.log |"));
});

test("buildPrometheusMetrics emits gauges and target details", () => {
  const summary = {
    generated_at: "2026-01-27T00:00:00Z",
    git_rev: "abc123",
    matrix_name: "demo",
    targets: [
      {
        label: "node20",
        status: "passed",
        duration_ms: 1200,
        node_version: "v20.11.0",
        log_file: "artifacts/matrix/node20.log",
        exit_code: 0,
      },
      {
        label: "node18",
        status: "failed",
        duration_ms: undefined,
        node_version: undefined,
        log_file: "artifacts/matrix/node18.log",
        exit_code: 1,
      },
    ],
  };
  const metrics = buildPrometheusMetrics(summary);
  assert.ok(
    metrics.includes(
      'js_release_matrix_targets_total{matrix_name="demo",git_rev="abc123"} 2',
    ),
  );
  assert.ok(
    metrics.includes(
      'js_release_matrix_target_duration_seconds{matrix_name="demo",git_rev="abc123",target="node20",status="passed",node_version="v20.11.0",log_file="artifacts/matrix/node20.log"} 1.2',
    ),
  );
  assert.ok(
    metrics.includes(
      'js_release_matrix_target_exit_code{matrix_name="demo",git_rev="abc123",target="node18",status="failed",log_file="artifacts/matrix/node18.log"} 1',
    ),
  );
});
