import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..");

const RUN_HEAVY = process.env.SORAFS_HEAVY === "1";

test(
  "SoraFS 1GiB digest parity via CLI",
  {
    skip: !RUN_HEAVY,
  },
  () => {
    const result = spawnSync(
      "cargo",
      ["run", "--quiet", "-p", "sorafs_chunker", "--bin", "sorafs_chunk_digest"],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: {
          ...process.env,
          CARGO_TERM_COLOR: "never",
          CARGO_NET_OFFLINE: process.env.CARGO_NET_OFFLINE ?? "true",
        },
      },
    );
    if (result.error) {
      throw result.error;
    }
    assert.equal(result.status, 0, result.stderr);
    let report;
    try {
      report = JSON.parse(result.stdout);
    } catch (error) {
      throw new Error(`failed to parse CLI JSON: ${error}\nstdout:\n${result.stdout}`);
    }
    assert.equal(report.profile, "sorafs.sf1@1.0.0");
    assert.equal(report.total_bytes, 1 << 30);
    assert.equal(report.overall_digest, "03200fc323fdc649ad7fc5a2d7949247db73a119de2da65971064a5e6b7e8a42");
    assert.ok(Array.isArray(report.samples), "samples array missing");
    const expectedSamples = [
      { index: 0, digest: "7789b490337d16c51b59a92e354a657ba450da4bab872c31e85e4d4fedcb3a27" },
      { index: Math.floor(report.chunk_count / 2), digest: "79f7fbd8c45dac13b1fa0be8ac366586f594b4ea7b079cc04814fabf43797bc4" },
      { index: report.chunk_count - 1, digest: "c35af9444dc6f7374909860b553433c222155d36d3d06c4398570d4a39e72d41" },
    ];
    assert.equal(report.samples.length, expectedSamples.length);
    expectedSamples.forEach((expected, idx) => {
      const actual = report.samples[idx];
      assert.equal(actual.index, expected.index);
      assert.equal(actual.digest, expected.digest);
    });
    assert.ok(report.chunk_count > 0);
    assert.ok(report.chunk_lengths);
    assert.equal(report.chunk_lengths.profile_min, 64 * 1024);
    assert.equal(report.chunk_lengths.profile_max, 512 * 1024);
  },
);
