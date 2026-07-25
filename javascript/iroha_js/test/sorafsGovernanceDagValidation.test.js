import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  SORAFS_GOVERNANCE_DAG_MAX_BLOCKS_V1,
  SORAFS_REFERENCE_MAX_LABEL_BYTES_V1,
  validateGovernanceDagBlock,
  validateGovernanceDagHeadChain,
} from "../src/sorafs.js";

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
  return JSON.parse(
    readFileSync(new URL(name, GOVERNANCE_FIXTURE_ROOT), "utf8"),
  );
}

test("validateGovernanceDagBlock accepts the canonical signed fixture", () => {
  const { root } = governanceFixtures();
  const outcome = validateGovernanceDagBlock(root, {
    label: "fixtures/sorafs_manifest/governance/dag_block_0_v1.to",
    generatedAtUnix: 1_700_002_001,
  });

  assert.equal(outcome.status, "Ok");
  assert.equal(outcome.code, "SFS-OK-000");
  assert.equal(outcome.generated_at, 1_700_002_001);
  assert.deepEqual(outcome.inputs, [
    {
      kind: "governance_dag_block",
      path: "fixtures/sorafs_manifest/governance/dag_block_0_v1.to",
    },
  ]);
});

test("validateGovernanceDagBlock rejects an expected CID mismatch", () => {
  const { root } = governanceFixtures();
  const outcome = validateGovernanceDagBlock(root, {
    expected_block_cid: Buffer.alloc(32),
    generated_at: 1_700_002_002,
  });

  assert.equal(outcome.status, "Error");
  assert.equal(outcome.code, "SFS-GOV-004");
  assert.equal(outcome.category, "validation");
  assert.equal(outcome.generated_at, 1_700_002_002);
  assert.deepEqual(outcome.inputs, [
    {
      kind: "governance_dag_block",
      path: "governance-dag-block.to",
    },
  ]);
});

test("validateGovernanceDagHeadChain accepts canonical root-to-head order", () => {
  const { root, child, head } = governanceFixtures();
  const outcome = validateGovernanceDagHeadChain(
    head,
    [
      {
        payload: root,
        label: "fixtures/sorafs_manifest/governance/dag_block_0_v1.to",
      },
      {
        bytes: child,
        label: "fixtures/sorafs_manifest/governance/dag_block_1_v1.to",
      },
    ],
    {
      headLabel: "fixtures/sorafs_manifest/governance/dag_head_v1.to",
      generatedAtUnix: 1_700_002_003,
    },
  );

  assert.equal(outcome.status, "Ok");
  assert.equal(outcome.code, "SFS-OK-000");
  assert.equal(outcome.generated_at, 1_700_002_003);
  assert.deepEqual(
    outcome.inputs.map((entry) => entry.kind),
    [
      "governance_dag_head",
      "governance_dag_block",
      "governance_dag_block",
    ],
  );
});

test("validateGovernanceDagHeadChain rejects reordered blocks", () => {
  const { root, child, head } = governanceFixtures();
  const outcome = validateGovernanceDagHeadChain(
    head,
    [
      { payload: child },
      { payload: root },
    ],
    { generatedAtUnix: 1_700_002_004 },
  );

  assert.equal(outcome.status, "Error");
  assert.equal(outcome.code, "SFS-GOV-006");
  assert.equal(outcome.generated_at, 1_700_002_004);
  assert.deepEqual(
    outcome.inputs.map((entry) => entry.path),
    [
      "governance-dag-head.to",
      "governance-dag-block-0.to",
      "governance-dag-block-1.to",
    ],
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
  assert.deepEqual(
    blockSignatureOutcome,
    governanceOutcomeFixture(
      "dag_block_bad_signature_validation_outcome_v1.json",
    ),
  );

  const trailingBytesOutcome = validateGovernanceDagBlock(
    governanceFixture("dag_block_trailing_bytes_v1.to"),
    {
      label: "dag_block_trailing_bytes_v1.to",
      generatedAtUnix: 123,
    },
  );
  assert.deepEqual(
    trailingBytesOutcome,
    governanceOutcomeFixture(
      "dag_block_trailing_bytes_validation_outcome_v1.json",
    ),
  );

  const headSignatureOutcome = validateGovernanceDagHeadChain(
    governanceFixture("dag_head_bad_signature_v1.to"),
    [
      { payload: root, label: "dag_block_0_v1.to" },
      { payload: child, label: "dag_block_1_v1.to" },
    ],
    {
      headLabel: "dag_head_bad_signature_v1.to",
      generatedAtUnix: 123,
    },
  );
  assert.deepEqual(
    headSignatureOutcome,
    governanceOutcomeFixture(
      "dag_head_bad_signature_validation_outcome_v1.json",
    ),
  );

  const predecessorOutcome = validateGovernanceDagHeadChain(
    governanceFixture("dag_head_bad_predecessor_v1.to"),
    [
      { payload: root, label: "dag_block_0_v1.to" },
      {
        payload: governanceFixture("dag_block_1_bad_predecessor_v1.to"),
        label: "dag_block_1_bad_predecessor_v1.to",
      },
    ],
    {
      headLabel: "dag_head_bad_predecessor_v1.to",
      generatedAtUnix: 123,
    },
  );
  assert.deepEqual(
    predecessorOutcome,
    governanceOutcomeFixture(
      "dag_head_bad_predecessor_validation_outcome_v1.json",
    ),
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
    () => validateGovernanceDagHeadChain(head, []),
    /1\.\.=64 entries/i,
  );
  assert.throws(
    () =>
      validateGovernanceDagHeadChain(
        head,
        Array.from(
          { length: SORAFS_GOVERNANCE_DAG_MAX_BLOCKS_V1 + 1 },
          () => ({ payload: root }),
        ),
      ),
    /1\.\.=64 entries/i,
  );
});
