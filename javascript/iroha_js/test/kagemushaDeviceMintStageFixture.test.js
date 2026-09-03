// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import { Kagemusha } from "../src/kagemusha.js";

function wire(section) {
  const bytes = Uint8Array.from(Buffer.from(section.hex, "hex"));
  assert.equal(bytes.length, section.raw_bytes);
  assert.equal(Buffer.from(bytes).toString("hex"), section.hex);
  return bytes;
}

test("operation-21 public bodies match the Rust-generated canonical fixture", () => {
  const fixture = JSON.parse(readFileSync(
    new URL("../../../fixtures/offline/kagemusha_device_mint_stage_v1.json", import.meta.url),
    "utf8",
  ));
  assert.equal(fixture.fixture_version, 1);
  assert.equal(fixture.protocol, "KAGEMUSHA");
  assert.equal(fixture.operation, 21);
  assert.equal(fixture.structural_only, true);
  assert.equal(fixture.command.schema,
    "iroha_data_model::kagemusha::kagemusha_device_v1::KagemushaDeviceMintStageCommandV1");
  assert.equal(fixture.command.alignment, 8);

  const authorizationBytes = wire(fixture.authorization);
  const authorization = Kagemusha.decodeMintAuthorization(authorizationBytes);
  assert.deepEqual(Kagemusha.encodeMintAuthorization(authorization), authorizationBytes);
  const creditBytes = wire(fixture.mint_credit);
  const credit = Kagemusha.decodeMintCredit(creditBytes, authorization);
  assert.deepEqual(Kagemusha.encodeMintCredit(credit, authorization), creditBytes);
  assert.equal(Buffer.from(credit.statement.lifecycle.creditId).toString("hex"), fixture.credit_id_hex);

  const commandBytes = wire(fixture.command);
  const command = Kagemusha.decodeDeviceMintStageCommandShapeExact(commandBytes);
  assert.deepEqual(command.canonicalAuthorization, authorizationBytes);
  assert.deepEqual(command.canonicalMintCredit, creditBytes);
  assert.deepEqual(Kagemusha.encodeDeviceMintStageCommandShape(command), commandBytes);
  assert.deepEqual(Kagemusha.encodeDeviceMintStageCommandShape(authorizationBytes, creditBytes), commandBytes);

  for (const [section, disposition] of [
    [fixture.staged_result, 0], [fixture.exact_duplicate_result, 1],
  ]) {
    assert.equal(section.schema,
      "iroha_data_model::kagemusha::kagemusha_device_v1::KagemushaDeviceMintStageResultV1");
    assert.equal(section.alignment, 2);
    const resultBytes = wire(section);
    const result = Kagemusha.decodeDeviceMintStageResultShapeExact(resultBytes, command);
    assert.equal(result.disposition, disposition);
    assert.equal(Buffer.from(result.creditId).toString("hex"), fixture.credit_id_hex);
    assert.deepEqual(Kagemusha.encodeDeviceMintStageResultShape(result, command), resultBytes);
  }
});
