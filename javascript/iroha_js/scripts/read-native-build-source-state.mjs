// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import process from "node:process";

import { readNativeBuildSourceState } from "./native-build-provenance.mjs";

if (
  (process.argv[2] !== "--" && process.argv[2] !== "--verify") ||
  (process.argv[2] === "--" && process.argv.length !== 4) ||
  (process.argv[2] === "--verify" && process.argv.length !== 5)
) {
  throw new TypeError(
    "read-native-build-source-state received invalid arguments",
  );
}

const state = readNativeBuildSourceState(process.argv[3]);
const keys = Object.keys(state).sort();
if (
  keys.join(",") !==
    "sourceGitRevision,sourceTreeClean,sourceTreeSha256" ||
  !/^[0-9a-f]{40}$/u.test(state.sourceGitRevision) ||
  typeof state.sourceTreeClean !== "boolean" ||
  !/^[0-9a-f]{64}$/u.test(state.sourceTreeSha256)
) {
  throw new Error("Native build source-state reader produced an invalid state");
}
if (process.argv[2] === "--verify") {
  const expected = JSON.parse(process.argv[4]);
  if (
    expected.sourceTreeClean !== false ||
    expected.cargoProfile !== "debug" ||
    state.sourceTreeClean !== false ||
    state.sourceGitRevision !== expected.sourceGitRevision ||
    state.sourceTreeSha256 !== expected.sourceTreeSha256
  ) {
    throw new Error("Native build source state does not match provenance");
  }
} else {
  process.stdout.write(JSON.stringify(state));
}
