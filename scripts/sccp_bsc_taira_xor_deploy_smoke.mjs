#!/usr/bin/env node
// Purpose: run a local end-to-end smoke test for the BSC SCCP deployment
// helper against an in-process Ganache chain configured as BSC testnet
// (`chainId = 97`). Safe default: this script never connects to public RPCs
// and never writes or prints private keys.
//
// Prerequisites:
// - Node.js 18+.
// - `ganache`, `solc`, and `ethers` on NODE_PATH.
import { createRequire } from "node:module";
import { writeSync } from "node:fs";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import {
  BSC_TESTNET_NETWORK_ID_HEX,
  main as deployMain,
} from "./sccp_bsc_taira_xor_deploy.mjs";

const requireFromScript = createRequire(import.meta.url);
const requireFromCwd = createRequire(`${resolve("noop.js")}`);

function requireOptionalPackage(name) {
  try {
    return requireFromScript(name);
  } catch (_firstError) {
    return requireFromCwd(name);
  }
}

const G1 = ["1", "2"];
const G2 = [
  "10857046999023057135944570762232829481370756359578518086990519993285655852781",
  "11559732032986387107991004021392285783925812861821192530917403151452391805634",
  "8495653923123431417604973247489272438418190587263600148770280649306958101930",
  "4082367875863433681332203403145435568316851327593401208105741076214120093531",
];
const LOCAL_IC = [
  "1368015179489954701390400359078579693043519447331113978918064868415326638035",
  "9918110051302171585080402603319702774565515993150576347155970296011118125764",
  "3353031288059533942658390886683067124040920775575537747144343083137631628272",
  "19321533766552368860946552437480515441416830039777911637913418824951667761761",
  "3010198690406615200373504922352659861758983907867017329644089018310584441462",
  "4027184618003122424972590350825261965929648733675738730716654005365300998076",
  "10744596414106452074759370245733544594153395043370666422502510773307029471145",
  "848677436511517736191562425154572367705380862894644942948681172815252343932",
  "4503322228978077916651710446042370109107355802721800704639343137502100212473",
  "6132642251294427119375180147349983541569387941788025780665104001559216576968",
  "10415861484417082502655338383609494480414113902179649885744799961447382638712",
  "10196215078179488638353184030336251401353352596818396260819493263908881608606",
  "3932705576657793550893430333273221375907985235130430286685735064194643946083",
  "18813763293032256545937756946359266117037834559191913266454084342712532869153",
  "1624070059937464756887933993293429854168590106605707304006200119738501412969",
  "3269329550605213075043232856820720631601935657990457502777101397807070461336",
  "4444740815889402603535294170722302758225367627362056425101568584910268024244",
  "10537263096529483164618820017164668921386457028564663708352735080900270541420",
  "19033251874843656108471242320417533909414939332036131356573128480367742634479",
  "20792135454608030201903199625673964159744755218442260092768620403349374102584",
];
const PRIVATE_KEY_ENV = "SCCP_BSC_LOCAL_SMOKE_PRIVATE_KEY";
const DEPLOY_TIMEOUT_MS = 180_000;
const CLOSE_TIMEOUT_MS = 15_000;

function logPhase(message) {
  writeSync(2, `sccp_bsc_taira_xor_deploy_smoke: ${message}\n`);
}

function withTimeout(promise, label, timeoutMs) {
  let timeout = null;
  const timeoutPromise = new Promise((_, reject) => {
    timeout = setTimeout(() => {
      reject(new Error(`${label} timed out after ${timeoutMs}ms`));
    }, timeoutMs);
  });
  return Promise.race([promise, timeoutPromise]).finally(() => {
    if (timeout) {
      clearTimeout(timeout);
    }
  });
}

function listen(server, port, host) {
  return new Promise((resolveListen, rejectListen) => {
    server.listen(port, host, (error) => {
      if (error) {
        rejectListen(error);
      } else {
        resolveListen(server.address());
      }
    });
  });
}

function close(server) {
  return new Promise((resolveClose, rejectClose) => {
    server.close((error) => {
      if (error) {
        rejectClose(error);
      } else {
        resolveClose();
      }
    });
  });
}

function localVerifierKeyHash(ethers, material) {
  const coder = ethers.AbiCoder.defaultAbiCoder();
  let encoded = coder.encode(
    [
      "uint256",
      "uint256",
      "uint256[2]",
      "uint256[2]",
      "uint256[2]",
      "uint256[2]",
      "uint256[2]",
      "uint256[2]",
    ],
    [
      material.alpha1[0],
      material.alpha1[1],
      material.beta2.slice(0, 2),
      material.beta2.slice(2, 4),
      material.gamma2.slice(0, 2),
      material.gamma2.slice(2, 4),
      material.delta2.slice(0, 2),
      material.delta2.slice(2, 4),
    ],
  );
  for (let index = 0; index < material.ic.length; index += 2) {
    encoded = ethers.concat([
      encoded,
      coder.encode(["uint256", "uint256"], [
        material.ic[index],
        material.ic[index + 1],
      ]),
    ]);
  }
  return ethers.keccak256(encoded);
}

async function main() {
  logPhase("starting local Ganache deployment smoke");
  const ganache = requireOptionalPackage("ganache");
  const ethers = requireOptionalPackage("ethers");
  const server = ganache.server({
    chain: {
      chainId: 97,
      networkId: 97,
    },
    logging: {
      quiet: true,
    },
    wallet: {
      deterministic: true,
      totalAccounts: 2,
      defaultBalance: 100,
    },
  });
  const workDir = await mkdtemp(join(tmpdir(), "iroha-sccp-bsc-deploy-smoke."));
  const previousPrivateKey = process.env[PRIVATE_KEY_ENV];
  try {
    const address = await listen(server, 0, "127.0.0.1");
    logPhase(`ganache listening on 127.0.0.1:${address.port}`);
    const endpoint = `http://127.0.0.1:${address.port}`;
    const initialAccounts = server.provider.getInitialAccounts();
    const [deployer] = Object.values(initialAccounts);
    process.env[PRIVATE_KEY_ENV] = deployer.secretKey;
    const verifierPath = join(workDir, "verifier.json");
    const evidencePath = join(workDir, "deployment.evidence.json");
    const verifierMaterial = {
      alpha1: G1,
      beta2: G2,
      gamma2: G2,
      delta2: G2,
      ic: LOCAL_IC,
      proofFamily: "stark-fri-v1",
      networkId: BSC_TESTNET_NETWORK_ID_HEX,
      sourceDomain: 0,
      targetDomain: 2,
    };
    await writeFile(
      verifierPath,
      `${JSON.stringify(
        {
          ...verifierMaterial,
          verifierKeyHash: localVerifierKeyHash(ethers, verifierMaterial),
        },
        null,
        2,
      )}\n`,
    );
    logPhase("deploying BSC SCCP contracts");
    const originalExit = process.exit;
    process.exit = ((code) => {
      throw new Error(
        `unexpected process.exit(${code ?? 0}) during local BSC deploy smoke`,
      );
    });
    let deployResult;
    try {
      deployResult = await withTimeout(
        deployMain([
          "deploy",
          "--verifier",
          verifierPath,
          "--broadcast",
          "true",
          "--confirm-testnet",
          "taira_bsc_xor",
          "--private-key-env",
          PRIVATE_KEY_ENV,
          "--rpc-url",
          endpoint,
          "--allow-local-rpc",
          "true",
          "--out",
          evidencePath,
        ]),
        "local BSC deploy helper",
        DEPLOY_TIMEOUT_MS,
      );
    } finally {
      process.exit = originalExit;
    }
    logPhase("validating deployment evidence");
    const evidence = JSON.parse(await readFile(evidencePath, "utf8"));
    if (deployResult.ok !== true) {
      throw new Error("deploy result was not ok");
    }
    if (evidence.routeId !== "taira_bsc_xor" || evidence.assetKey !== "xor") {
      throw new Error("deployment evidence route or asset mismatch");
    }
    if (evidence.bscContractReadback?.tokenBridgeLocked !== true) {
      throw new Error("deployment evidence did not lock TairaXOR bridge");
    }
    if (
      evidence.bscContractReadback?.sourceBridgeOwner !==
      evidence.bscBridgeAddress
    ) {
      throw new Error("source bridge owner does not match route bridge");
    }
    const serializedEvidence = JSON.stringify(evidence);
    if (/private[_-]?key|mnemonic|seed|secret/iu.test(serializedEvidence)) {
      throw new Error("deployment evidence leaked secret-like material");
    }
    const summary = {
      ok: true,
      bscBridgeAddress: evidence.bscBridgeAddress,
      bscTokenAddress: evidence.bscTokenAddress,
      sccpBscSourceBridgeAddress: evidence.sccpBscSourceBridgeAddress,
      bscVerifierAddress: evidence.bscVerifierAddress,
      destinationBindingHash:
        evidence.destinationRollout.destinationBindingHash,
    };
    const summaryText = `${JSON.stringify(summary, null, 2)}\n`;
    writeSync(1, summaryText);
    writeSync(
      2,
      `sccp_bsc_taira_xor_deploy_smoke: ${JSON.stringify(summary)}\n`,
    );
  } finally {
    if (previousPrivateKey === undefined) {
      delete process.env[PRIVATE_KEY_ENV];
    } else {
      process.env[PRIVATE_KEY_ENV] = previousPrivateKey;
    }
    await withTimeout(close(server), "ganache server close", CLOSE_TIMEOUT_MS).catch(
      () => {},
    );
    await rm(workDir, { recursive: true, force: true });
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
