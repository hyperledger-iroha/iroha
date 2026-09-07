import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import { encodeRaceValueV1, decodeRaceValueV1, raceGameplayHashV1, raceInputPayloadV1, raceControlsFromPayloadV1 } from "../src/race.js";
const fixture = JSON.parse(fs.readFileSync(new URL("fixtures/race-v1-codec.json", import.meta.url), "utf8"));
for (const row of fixture.vectors) test(`SORA CARS adapter canonical ${row.name} fixture`, () => {
  const encoded = encodeRaceValueV1(row.name, row.value);
  assert.equal(Buffer.from(encoded).toString("hex").toUpperCase(), row.encoded_hex);
  assert.deepEqual(encodeRaceValueV1(row.name, decodeRaceValueV1(row.name, encoded)), encoded);
  if (row.domain) assert.equal(Buffer.from(raceGameplayHashV1(fixture.network_id, row.domain, row.value)).toString("hex").toUpperCase(), row.gameplay_digest_hex);
});
test("SORA CARS adapter converts only canonical six-word batches into opaque generic inputs", () => {
  const controls = [1, 33, 9, 5, 17, 2];
  assert.deepEqual(raceInputPayloadV1(controls), [1, 0, 33, 0, 9, 0, 5, 0, 17, 0, 2, 0]);
  assert.deepEqual(raceControlsFromPayloadV1(raceInputPayloadV1(controls)), controls);
  assert.throws(() => raceInputPayloadV1([64, 0, 0, 0, 0, 0]));
  assert.throws(() => raceControlsFromPayloadV1([1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]));
  assert.throws(() => encodeRaceValueV1("OpenRaceV1", {}), /unknown/);
});
