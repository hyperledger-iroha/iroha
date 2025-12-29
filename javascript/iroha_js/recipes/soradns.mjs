#!/usr/bin/env node
/**
 * Deterministic SoraDNS gateway host walkthrough.
 *
 * Derives the canonical + pretty hosts for a supplied FQDN using the SDK’s
 * Blake3-backed helper and optionally validates a comma-separated list of
 * GAR host patterns. This mirrors the resolver logic described in
 * docs/source/soradns/deterministic_hosts.md.
 *
 * Usage:
 *   npm install
 *   npm run build:native
 *   node ./recipes/soradns.mjs docs.sora --gar-patterns canonical,pretty,*.gw.sora.id
 */
import process from "node:process";
import {
  deriveSoradnsGatewayHosts,
  hostPatternsCoverDerivedHosts,
} from "../src/index.js";

function parseArgs(argv) {
  const args = [...argv];
  let garArg;
  let fqdn;
  for (let idx = 0; idx < args.length; idx += 1) {
    const value = args[idx];
    if (value === "--gar-patterns") {
      garArg = args[idx + 1];
      args.splice(idx, 2);
      break;
    }
  }
  if (args.length > 0) {
    [fqdn] = args;
  }
  const name = fqdn ?? process.env.SORADNS_NAME ?? "docs.sora";
  const patternsSource = garArg ?? process.env.SORADNS_GAR ?? "";
  const garPatterns =
    patternsSource === ""
      ? []
      : patternsSource
          .split(/[,\n]/)
          .map((entry) => entry.trim())
          .filter((entry) => entry.length > 0);
  return { name, garPatterns };
}

async function main() {
  const { name, garPatterns } = parseArgs(process.argv.slice(2));
  console.log(`Deriving deterministic hosts for: ${name}`);
  let bindings;
  try {
    bindings = deriveSoradnsGatewayHosts(name);
  } catch (error) {
    console.error("Failed to derive gateway hosts:", error?.message ?? error);
    process.exitCode = 1;
    return;
  }

  console.log("Normalized name :", bindings.normalizedName);
  console.log("Canonical label :", bindings.canonicalLabel);
  console.log("Canonical host  :", bindings.canonicalHost);
  console.log("Pretty host     :", bindings.prettyHost);
  console.log("Wildcard        :", bindings.canonicalWildcard);
  console.log("GAR patterns    :");
  bindings.hostPatterns.forEach((pattern) => console.log(`  - ${pattern}`));

  if (garPatterns.length > 0) {
    const covered = hostPatternsCoverDerivedHosts(garPatterns, bindings);
    console.log(
      `\nGAR coverage (${garPatterns.join(", ")}): ${covered ? "ok" : "missing required host(s)"}`,
    );
    if (!covered) {
      console.log(
        "Ensure the GAR authorises the canonical host, canonical wildcard, and pretty host.",
      );
      process.exitCode = 2;
    }
  } else {
    console.log(
      "\nTip: pass --gar-patterns host1,host2,host3 (or set SORADNS_GAR) to validate a live GAR payload.",
    );
  }
}

await main();
