import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..");
const fixturesDir = path.join(repoRoot, "fixtures", "sorafs_chunker");
const fuzzDir = path.join(repoRoot, "fuzz", "sorafs_chunker");

function loadJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

test("SoraFS chunker fixture matches canonical structure", () => {
  const fixturePath = path.join(fixturesDir, "sf1_profile_v1.json");
  const fixture = loadJson(fixturePath);

  assert.equal(fixture.profile, "sorafs.sf1@1.0.0");
  assert.equal(fixture.input_length, 1 << 20);
  assert.equal(fixture.chunk_count, fixture.chunk_lengths.length);
  assert.equal(fixture.chunk_count, fixture.chunk_offsets.length);
  assert.equal(fixture.chunk_count, fixture.chunk_digests_blake3.length);

  const total = fixture.chunk_lengths.reduce((sum, value) => sum + value, 0);
  assert.equal(total, fixture.input_length);

  fixture.chunk_offsets.forEach((offset, index) => {
    assert.ok(offset >= 0, "chunk offset should be non-negative");
    if (index === 0) {
      assert.equal(offset, 0);
    } else {
      const prevEnd = fixture.chunk_offsets[index - 1] + fixture.chunk_lengths[index - 1];
      assert.equal(offset, prevEnd, `chunk offset mismatch at index ${index}`);
    }
  });

  const inputPath = path.join(fuzzDir, "sf1_profile_v1_input.bin");
  const stats = fs.statSync(inputPath);
  assert.equal(stats.size, fixture.input_length);
});

test("Back-pressure scenarios mirror canonical chunking", () => {
  const scenarioPath = path.join(fuzzDir, "sf1_profile_v1_backpressure.json");
  const scenarios = loadJson(scenarioPath);
  const fixture = loadJson(path.join(fixturesDir, "sf1_profile_v1.json"));

  assert.equal(scenarios.profile, fixture.profile);
  assert.equal(scenarios.input_length, fixture.input_length);
  assert.equal(scenarios.chunk_digest_sha3_256, fixture.chunk_digest_sha3_256);

  for (const scenario of scenarios.scenarios) {
    const feedSum = scenario.feed_sizes.reduce((sum, value) => sum + value, 0);
    assert.equal(feedSum, scenarios.input_length, `feed size sum mismatch for ${scenario.name}`);
    assert.equal(
      scenario.expected_chunk_lengths.length,
      fixture.chunk_lengths.length,
      `chunk length count mismatch for ${scenario.name}`,
    );
    assert.equal(
      scenario.chunk_count,
      fixture.chunk_count,
      `chunk count mismatch for ${scenario.name}`,
    );
    scenario.expected_chunk_lengths.forEach((length, index) => {
      assert.equal(
        length,
        fixture.chunk_lengths[index],
        `chunk length mismatch for ${scenario.name} at index ${index}`,
      );
    });
    const maxFeed = Math.max(...scenario.feed_sizes);
    const minFeed = Math.min(...scenario.feed_sizes);
    assert.equal(
      scenario.max_feed_size,
      maxFeed,
      `max feed size mismatch for ${scenario.name}`,
    );
    assert.equal(
      scenario.min_feed_size,
      minFeed,
      `min feed size mismatch for ${scenario.name}`,
    );
  }
});
