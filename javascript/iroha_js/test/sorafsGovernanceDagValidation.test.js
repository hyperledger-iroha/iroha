import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";

import {
  SORAFS_GOVERNANCE_DAG_CID_BYTES_V1,
  SORAFS_GOVERNANCE_DAG_MAX_BLOCKS_V1,
  SORAFS_REFERENCE_MAX_LABEL_BYTES_V1,
  validateGovernanceLogNode,
  validateGovernanceDagBlock,
  validateGovernanceDagHeadChain,
} from "../src/sorafs.js";
import { makeNativeTest } from "./helpers/native.js";

const ROOT_BLOCK_FIXTURE = new URL(
  "../../../fixtures/sorafs_manifest/governance/dag_block_0_v1.to",
  import.meta.url,
);
const CHILD_BLOCK_FIXTURE = new URL(
  "../../../fixtures/sorafs_manifest/governance/dag_block_1_v1.to",
  import.meta.url,
);
const HEAD_FIXTURE = new URL(
  "../../../fixtures/sorafs_manifest/governance/dag_head_v1.to",
  import.meta.url,
);
const GOVERNANCE_FIXTURE_ROOT = new URL(
  "../../../fixtures/sorafs_manifest/governance/",
  import.meta.url,
);
const MODERATION_FIXTURE_ROOT = new URL(
  "../../../fixtures/sorafs_manifest/moderation/",
  import.meta.url,
);
const governanceLogNodeNativeTest = makeNativeTest(test, {
  require: "sorafsValidateGovernanceLogNodeJson",
});

function governanceFixtures() {
  return {
    root: readFileSync(ROOT_BLOCK_FIXTURE),
    child: readFileSync(CHILD_BLOCK_FIXTURE),
    head: readFileSync(HEAD_FIXTURE),
  };
}

function governanceFixture(name) {
  return readFileSync(new URL(name, GOVERNANCE_FIXTURE_ROOT));
}

function governanceOutcomeFixture(name) {
  return JSON.parse(governanceOutcomeFixtureText(name));
}

function governanceOutcomeFixtureText(name) {
  return readFileSync(new URL(name, GOVERNANCE_FIXTURE_ROOT), "utf8");
}

function assertGovernanceOutcome(outcome, fixtureName) {
  const expectedText = governanceOutcomeFixtureText(fixtureName);
  assert.deepEqual(outcome, governanceOutcomeFixture(fixtureName));
  assert.equal(`${JSON.stringify(outcome, null, 2)}\n`, expectedText);
}

governanceLogNodeNativeTest(
  "validateGovernanceLogNode matches the moderation golden byte-for-byte",
  () => {
    const nodeJson = JSON.parse(
      readFileSync(new URL("governance_node_v1.json", MODERATION_FIXTURE_ROOT), "utf8"),
    );
    const outcome = validateGovernanceLogNode(
      readFileSync(new URL("governance_node_v1.to", MODERATION_FIXTURE_ROOT)),
      {
        label: "moderation/governance_node_v1.to",
        expectedNodeCid: Buffer.from(nodeJson.node_cid_hex, "hex"),
        generatedAtUnix: 1_700_001_234,
      },
    );
    const expectedText = readFileSync(
      new URL(
        "governance_node_validation_outcome_v1.json",
        MODERATION_FIXTURE_ROOT,
      ),
      "utf8",
    );

    assert.deepEqual(outcome, JSON.parse(expectedText));
    assert.equal(`${JSON.stringify(outcome, null, 2)}\n`, expectedText);
  },
);

test("validateGovernanceLogNode requires the canonical expected node CID field", () => {
  assert.throws(
    () => validateGovernanceLogNode(Buffer.alloc(1)),
    /options must be an object/i,
  );
  assert.throws(
    () => validateGovernanceLogNode(Buffer.alloc(1), {}),
    /expectedNodeCid is required/i,
  );
  assert.throws(
    () =>
      validateGovernanceLogNode(Buffer.alloc(1), {
        expected_node_cid: Buffer.alloc(32),
      }),
    /unsupported fields/i,
  );
  for (const invalidLength of [0, 31, 33]) {
    assert.throws(
      () =>
        validateGovernanceLogNode(Buffer.alloc(1), {
          expectedNodeCid: Buffer.alloc(invalidLength),
        }),
      new RegExp(
        `exactly ${SORAFS_GOVERNANCE_DAG_CID_BYTES_V1} bytes`,
        "i",
      ),
    );
  }
});

test("validateGovernanceLogNode fails closed when the native module is missing", () => {
  const emptyNativeDirectory = mkdtempSync(
    join(tmpdir(), "iroha-js-governance-native-missing-"),
  );
  const moduleUrl = new URL("../src/sorafs.js", import.meta.url).href;
  const script = `
    import { validateGovernanceLogNode } from ${JSON.stringify(moduleUrl)};
    try {
      validateGovernanceLogNode(new Uint8Array([0]), {
        expectedNodeCid: new Uint8Array(32),
        generatedAtUnix: 1,
      });
    } catch (error) {
      if (error?.code === "ERR_IROHA_NATIVE_BINDING") {
        process.exit(0);
      }
      console.error(error?.stack ?? error);
      process.exit(2);
    }
    process.exit(3);
  `;
  try {
    const result = spawnSync(
      process.execPath,
      ["--input-type=module", "--eval", script],
      {
        encoding: "utf8",
        env: {
          ...process.env,
          IROHA_JS_NATIVE_DIR: emptyNativeDirectory,
        },
      },
    );
    assert.equal(
      result.status,
      0,
      `missing-native subprocess failed:\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}`,
    );
  } finally {
    rmSync(emptyNativeDirectory, { recursive: true, force: true });
  }
});

test("validateGovernanceDagBlock accepts the canonical signed fixture", () => {
  const { root } = governanceFixtures();
  const outcome = validateGovernanceDagBlock(root, {
    label: "dag_block_0_v1.to",
    generatedAtUnix: 123,
  });

  assertGovernanceOutcome(
    outcome,
    "dag_block_validation_outcome_v1.json",
  );
});

test("validateGovernanceDagBlock rejects an expected CID mismatch", () => {
  const { root } = governanceFixtures();
  const outcome = validateGovernanceDagBlock(root, {
    expectedBlockCid: Buffer.alloc(32, 0x7f),
    generatedAtUnix: 123,
  });

  assertGovernanceOutcome(
    outcome,
    "dag_block_cid_mismatch_validation_outcome_v1.json",
  );
});

test("validateGovernanceDagHeadChain accepts canonical root-to-head order", () => {
  const { root, child, head } = governanceFixtures();
  const outcome = validateGovernanceDagHeadChain(
    head,
    [
      {
        bytes: root,
        label: "dag_block_0_v1.to",
      },
      {
        bytes: child,
        label: "dag_block_1_v1.to",
      },
    ],
    {
      headLabel: "dag_head_v1.to",
      generatedAtUnix: 123,
    },
  );

  assertGovernanceOutcome(
    outcome,
    "dag_head_validation_outcome_v1.json",
  );
});

test("validateGovernanceDagHeadChain rejects reordered blocks", () => {
  const { root, child, head } = governanceFixtures();
  const outcome = validateGovernanceDagHeadChain(
    head,
    [
      { bytes: child },
      { bytes: root },
    ],
    { generatedAtUnix: 123 },
  );

  assertGovernanceOutcome(
    outcome,
    "dag_head_reordered_validation_outcome_v1.json",
  );
});

test("governance DAG validators match canonical noncanonical, signature, and predecessor vectors", () => {
  const { root, child } = governanceFixtures();
  const blockSignatureOutcome = validateGovernanceDagBlock(
    governanceFixture("dag_block_bad_signature_v1.to"),
    {
      label: "dag_block_bad_signature_v1.to",
      generatedAtUnix: 123,
    },
  );
  assertGovernanceOutcome(
    blockSignatureOutcome,
    "dag_block_bad_signature_validation_outcome_v1.json",
  );

  const trailingBytesOutcome = validateGovernanceDagBlock(
    governanceFixture("dag_block_trailing_bytes_v1.to"),
    {
      label: "dag_block_trailing_bytes_v1.to",
      generatedAtUnix: 123,
    },
  );
  assertGovernanceOutcome(
    trailingBytesOutcome,
    "dag_block_trailing_bytes_validation_outcome_v1.json",
  );

  const headSignatureOutcome = validateGovernanceDagHeadChain(
    governanceFixture("dag_head_bad_signature_v1.to"),
    [
      { bytes: root, label: "dag_block_0_v1.to" },
      { bytes: child, label: "dag_block_1_v1.to" },
    ],
    {
      headLabel: "dag_head_bad_signature_v1.to",
      generatedAtUnix: 123,
    },
  );
  assertGovernanceOutcome(
    headSignatureOutcome,
    "dag_head_bad_signature_validation_outcome_v1.json",
  );

  const predecessorOutcome = validateGovernanceDagHeadChain(
    governanceFixture("dag_head_bad_predecessor_v1.to"),
    [
      { bytes: root, label: "dag_block_0_v1.to" },
      {
        bytes: governanceFixture("dag_block_1_bad_predecessor_v1.to"),
        label: "dag_block_1_bad_predecessor_v1.to",
      },
    ],
    {
      headLabel: "dag_head_bad_predecessor_v1.to",
      generatedAtUnix: 123,
    },
  );
  assertGovernanceOutcome(
    predecessorOutcome,
    "dag_head_bad_predecessor_validation_outcome_v1.json",
  );
});

test("governance DAG wrappers reject retired field aliases before native dispatch", () => {
  const bytes = Buffer.alloc(1);
  assert.throws(
    () =>
      validateGovernanceLogNode(bytes, {
        expectedNodeCid: Buffer.alloc(32),
        generated_at: 1,
      }),
    /unsupported fields/i,
  );
  for (const options of [
    { expected_block_cid: Buffer.alloc(32) },
    { generated_at: 1 },
  ]) {
    assert.throws(
      () => validateGovernanceDagBlock(bytes, options),
      /unsupported fields/i,
    );
  }
  for (const block of [
    { payload: bytes },
    { noritoBytes: bytes },
    { norito_bytes: bytes },
  ]) {
    assert.throws(
      () => validateGovernanceDagHeadChain(bytes, [block]),
      /unsupported fields/i,
    );
  }
  assert.throws(
    () => validateGovernanceDagHeadChain(bytes, [{ bytes }], { head_label: "head.to" }),
    /unsupported fields/i,
  );
});

test("governance DAG wrappers enforce label and block-count bounds", () => {
  const { root, head } = governanceFixtures();
  assert.throws(
    () =>
      validateGovernanceDagBlock(root, {
        label: "x".repeat(SORAFS_REFERENCE_MAX_LABEL_BYTES_V1 + 1),
      }),
    /UTF-8 bytes/i,
  );
  assert.throws(
    () => validateGovernanceDagBlock(root, { label: "bad\u0001label" }),
    /control characters/i,
  );
  for (const invalidLength of [0, 31, 33]) {
    assert.throws(
      () =>
        validateGovernanceDagBlock(root, {
          expectedBlockCid: Buffer.alloc(invalidLength),
        }),
      new RegExp(
        `exactly ${SORAFS_GOVERNANCE_DAG_CID_BYTES_V1} bytes`,
        "i",
      ),
    );
  }
  assert.throws(
    () => validateGovernanceDagHeadChain(head, []),
    /1\.\.=64 entries/i,
  );
  assert.throws(
    () =>
      validateGovernanceDagHeadChain(
        head,
        Array.from(
          { length: SORAFS_GOVERNANCE_DAG_MAX_BLOCKS_V1 + 1 },
          () => ({ bytes: root }),
        ),
      ),
    /1\.\.=64 entries/i,
  );
});
