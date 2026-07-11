#!/usr/bin/env node
"use strict";

const fs = require("fs");
const path = require("path");

if (process.argv.length !== 5) {
  process.stderr.write("usage: contract_soljson_runner.js SOLJSON SHA256 EXPECTED_VERSION\n");
  process.exit(2);
}

const compilerPath = path.resolve(process.argv[2]);
process.env.SCCP_SOLJSON_PATH = compilerPath;
process.env.SCCP_SOLJSON_SHA256 = process.argv[3];

try {
  const solc = require("./contract_tooling/authenticated-solc");
  const version = solc.version();
  if (version !== process.argv[4]) {
    throw new Error("authenticated compiler version does not match its locked identity");
  }
  const input = fs.readFileSync(0, "utf8");
  if (input.length === 0 || input.length > 16 * 1024 * 1024) {
    throw new Error("standard-json compiler input is empty or exceeds 16 MiB");
  }
  const compilerOutput = solc.compile(input);
  const parsed = JSON.parse(compilerOutput);
  process.stdout.write(JSON.stringify({ compiler_version: version, output: parsed }));
} catch (error) {
  const message = error instanceof Error ? error.message : "unknown compiler runner failure";
  process.stderr.write(`authenticated soljson runner failed: ${message}\n`);
  process.exit(1);
}
