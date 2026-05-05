#!/usr/bin/env node

import {
  AccountAddress,
  ToriiClient,
  buildClaimTwitterFollowRewardInstruction,
  buildSendToTwitterInstruction,
  buildTransaction,
  generateKeyPair,
  publicKeyFromPrivate,
  submitSignedTransaction,
} from "../src/index.js";

function parseArgs(argv) {
  const result = new Map();
  for (let i = 0; i < argv.length; i += 1) {
    const key = argv[i];
    const next = argv[i + 1];
    if (key.startsWith("--")) {
      if (next && !next.startsWith("--")) {
        result.set(key, next);
        i += 1;
      } else {
        result.set(key, true);
      }
    }
  }
  return result;
}

function requireValue(map, key, fallback) {
  if (map.has(key)) {
    return map.get(key);
  }
  if (fallback !== undefined && fallback !== null) {
    return fallback;
  }
  throw new Error(`Missing required argument: ${key}`);
}

function asBuffer(hexString, context) {
  try {
    return Buffer.from(hexString, "hex");
  } catch (error) {
    throw new Error(`Invalid hex for ${context}: ${error.message}`);
  }
}

function normalizePrivateKeySeed(buffer) {
  if (buffer.length === 32) {
    return buffer;
  }
  const marker = buffer.indexOf(Buffer.from([0x04, 0x20]));
  if (marker !== -1 && marker + 34 <= buffer.length) {
    return buffer.subarray(marker + 2, marker + 34);
  }
  if (buffer.length > 64) {
    return buffer.subarray(buffer.length - 32);
  }
  return buffer;
}

function normalizeBinding(raw) {
  const trimmed = String(raw).trim();
  if (trimmed.startsWith("{")) {
    try {
      return JSON.parse(trimmed);
    } catch (error) {
      throw new Error(`Unable to parse binding JSON: ${error.message}`);
    }
  }
  return trimmed;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const toriiUrl = requireValue(args, "--torii", process.env.TORII_URL ?? "http://localhost:8080");
  const chainId = requireValue(args, "--chain-id", process.env.CHAIN_ID ?? "dev-chain");
  const bindingRaw = requireValue(args, "--binding", process.env.BINDING_HASH);
  const amount = requireValue(args, "--amount", process.env.SEND_AMOUNT ?? "10");
  const sessionAccount = requireValue(
    args,
    "--session-account",
    process.env.SESSION_ACCOUNT ?? "walletless-session",
  );
  const dryRun = args.has("--dry-run") || String(process.env.WALLETLESS_DRY_RUN ?? "").length > 0;

  const binding = normalizeBinding(bindingRaw);

  let sessionPrivate;
  let sessionPublicHex;
  if (args.has("--session-key") || process.env.SESSION_KEY_HEX) {
    sessionPrivate = normalizePrivateKeySeed(
      asBuffer(requireValue(args, "--session-key", process.env.SESSION_KEY_HEX), "session key"),
    );
  } else {
    const generated = generateKeyPair();
    sessionPrivate = normalizePrivateKeySeed(generated.privateKey);
    sessionPublicHex = generated.publicKey.toString("hex");
  }

  const sponsorId =
    trimToNull(args.get("--sponsor")) ??
    trimToNull(process.env.SPONSOR_ID) ??
    AccountAddress.fromAccount({ publicKey: publicKeyFromPrivate(sessionPrivate),
    }).toI105();

  const instructions = [
    buildSendToTwitterInstruction({ bindingHash: binding, amount }),
    buildClaimTwitterFollowRewardInstruction({ bindingHash: binding }),
  ];

  const metadata = { session_account: sessionAccount };
  if (sessionPublicHex) {
    metadata.walletless_session_pubkey_hex = sessionPublicHex;
  }

  const tx = buildTransaction({
    chainId,
    authority: sponsorId,
    instructions,
    metadata,
    privateKey: sessionPrivate,
  });

  console.log(`Walletless flow built for ${bindingRaw}`);
  console.log(`Transaction hash: ${tx.hash.toString("hex")}`);

  if (dryRun) {
    console.log("Dry-run enabled; not submitting to Torii.");
    return;
  }

  const client = new ToriiClient(toriiUrl);
  const result = await submitSignedTransaction(client, tx.signedTransaction, {
    waitForCommit: true,
  });
  console.log(`Submitted to ${toriiUrl}`);
  console.log(`Hash: ${result.hash}`);
  if (result.status) {
    console.log(`Final status: ${result.status?.content?.status?.kind ?? "unknown"}`);
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});

function trimToNull(value) {
  if (value === undefined || value === null) {
    return null;
  }
  const trimmed = String(value).trim();
  return trimmed.length === 0 ? null : trimmed;
}
