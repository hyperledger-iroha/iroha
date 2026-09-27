import assert from "node:assert/strict";
import test from "node:test";

import { AccountAddress } from "../src/address.js";
import {
  buildActivateRetailDailyLimitV1InstructionJson,
  buildBindRetailIdentityV1InstructionJson,
  buildRetailMonetaryMovementV1InstructionJson,
} from "../src/instructionBuilders.js";
import { _createNoritoInstructionApi } from "../src/norito.js";
import { createNativeRuntime } from "../src/nativeRuntime.js";
import { parseStrictLosslessIntegerJson } from "../src/strictLosslessJson.js";
import { hasNoritoBinding } from "./helpers/native.js";

const DATA_SPACE_ID = "8648377547929788715";
const ASSET_DEFINITION_ID = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
const builderTest = hasNoritoBinding() ? test : test.skip;
const ACCOUNT = hasNoritoBinding()
  ? AccountAddress.fromAccount({
      publicKey: Buffer.from("D04AB232742BB4AB3A1368BD4615E4E6D0224AB71A016BAF8520A332C9778737", "hex"),
    }).toI105(0x2f1)
  : null;
const RESERVE = hasNoritoBinding()
  ? AccountAddress.fromAccount({
      publicKey: Buffer.from("641297079357229F295938A4B5A333DE35069BF47B9D0704E45805713D13C201", "hex"),
    }).toI105(0x2f1)
  : null;
const digest = (byte) => Array(32).fill(byte);
const decode = (source) => parseStrictLosslessIntegerJson(source, "retail instruction test");

function activation() {
  return {
    definition: {
      id: ASSET_DEFINITION_ID,
      name: "Kina",
      description: null,
      alias: null,
      spec: { scale: 2 },
      mintable: "Infinitely",
      logo: null,
      metadata: {},
      balance_scope_policy: "DataspaceRestricted",
      owning_domain: "retail.bpng",
    },
    policy: {
      asset_definition_id: ASSET_DEFINITION_ID,
      physical_dataspace: DATA_SPACE_ID,
      revision: "1",
      daily_cap: "5",
      identity_issuer: ACCOUNT,
      identity_issuer_public_key: "ED0120D04AB232742BB4AB3A1368BD4615E4E6D0224AB71A016BAF8520A332C9778737",
      monetary_issuer_account: ACCOUNT,
      reserve_account: RESERVE,
      institutional_exceptions: [],
    },
  };
}

function binding() {
  return {
    attestation: {
      body: {
        domain: "iroha.bpng.retail-identity.v1",
        asset_definition_id: ASSET_DEFINITION_ID,
        physical_dataspace: DATA_SPACE_ID,
        policy_revision: "1",
        account_id: ACCOUNT,
        identity: { digest: digest(0xA1) },
        uniqueness_evidence_digest: digest(0xB2),
      },
      signature: "AB".repeat(64),
    },
  };
}

test("first-release revision and PGK minor-unit scale reject before native encode", () => {
  for (const revision of [2, "2", 2n]) {
    const wrongActivation = activation();
    wrongActivation.policy.revision = revision;
    assert.throws(
      () => buildActivateRetailDailyLimitV1InstructionJson(wrongActivation),
      /revision must be exactly 1 in the first release/u,
    );
    const wrongBinding = binding();
    wrongBinding.attestation.body.policy_revision = revision;
    assert.throws(
      () => buildBindRetailIdentityV1InstructionJson(wrongBinding),
      /policy_revision must be exactly 1 in the first release/u,
    );
  }

  const monetary = {
    assetDefinitionId: ASSET_DEFINITION_ID,
    purpose: "mint_to_reserve",
    retailAccount: null,
    amount: "0.01",
    operationDigest: digest(0xC3),
  };
  const accepted = decode(buildRetailMonetaryMovementV1InstructionJson(monetary));
  assert.equal(accepted.RetailMonetaryMovementV1.amount, "0.01");
  for (const amount of ["0.001", "1.234", "5.000"]) {
    assert.throws(
      () => buildRetailMonetaryMovementV1InstructionJson({ ...monetary, amount }),
      /exceeds PGK's scale 2|canonical quantity/u,
    );
  }
});

builderTest("retail activation keeps the exact large dataspace integer and closed policy", () => {
  const source = buildActivateRetailDailyLimitV1InstructionJson(activation());
  assert.match(source, /"physical_dataspace":8648377547929788715/u);
  const body = decode(source).ActivateRetailDailyLimitV1;
  assert.equal(body.policy.physical_dataspace, 8648377547929788715n);
  assert.equal(body.policy.revision, 1);
  assert.equal(body.definition.spec.scale, 2);
  assert.deepEqual(body.policy.institutional_exceptions, []);

  const wrongScale = activation();
  wrongScale.definition.spec.scale = 0;
  assert.throws(() => buildActivateRetailDailyLimitV1InstructionJson(wrongScale), /scale 2/u);
  const wrongId = activation();
  wrongId.policy.asset_definition_id = "61CtjvNd9T3THAR65GsMVHr82Bjc";
  assert.throws(() => buildActivateRetailDailyLimitV1InstructionJson(wrongId), /asset IDs disagree/u);
  const exception = activation();
  exception.policy.institutional_exceptions.push({ source_account: ACCOUNT });
  assert.throws(() => buildActivateRetailDailyLimitV1InstructionJson(exception), /no institutional exceptions/u);
  const rounded = activation();
  rounded.policy.physical_dataspace = Number(DATA_SPACE_ID);
  assert.throws(() => buildActivateRetailDailyLimitV1InstructionJson(rounded), /positive exact u64/u);
  const alias = activation();
  alias.policy.legacy_cap = "5";
  assert.throws(() => buildActivateRetailDailyLimitV1InstructionJson(alias), /must contain exactly/u);
});

builderTest("retail binding retains signed body bytes and exact identity commitment", () => {
  const source = buildBindRetailIdentityV1InstructionJson(binding());
  const body = decode(source).BindRetailIdentityV1.attestation.body;
  assert.equal(body.physical_dataspace, 8648377547929788715n);
  assert.deepEqual(body.identity.digest, digest(0xA1));
  assert.deepEqual(body.uniqueness_evidence_digest, digest(0xB2));

  const zero = binding();
  zero.attestation.body.identity.digest = digest(0);
  assert.throws(() => buildBindRetailIdentityV1InstructionJson(zero), /must be nonzero/u);
  const alternate = binding();
  alternate.attestation.signature = "ab".repeat(64);
  assert.throws(() => buildBindRetailIdentityV1InstructionJson(alternate), /uppercase hex/u);
  const omitted = binding();
  delete omitted.attestation.body.uniqueness_evidence_digest;
  assert.throws(() => buildBindRetailIdentityV1InstructionJson(omitted), /must contain exactly/u);
});

builderTest("retail monetary builder closes four purposes and account direction", () => {
  for (const purpose of ["mint_to_reserve", "credit_retail", "defund_retail", "burn_reserve"]) {
    const retailAccount = purpose === "credit_retail" || purpose === "defund_retail" ? ACCOUNT : null;
    const source = buildRetailMonetaryMovementV1InstructionJson({
      assetDefinitionId: ASSET_DEFINITION_ID,
      purpose,
      retailAccount,
      amount: "1.25",
      operationDigest: digest(0xC3),
    });
    const body = decode(source).RetailMonetaryMovementV1;
    assert.deepEqual(body.purpose, { purpose, value: null });
    assert.equal(body.retail_account, retailAccount);
    assert.equal(body.amount, "1.25");
  }
  const base = {
    assetDefinitionId: ASSET_DEFINITION_ID,
    purpose: "credit_retail",
    retailAccount: ACCOUNT,
    amount: "1.25",
    operationDigest: digest(0xC3),
  };
  assert.throws(() => buildRetailMonetaryMovementV1InstructionJson({ ...base, purpose: "top_up" }), /not a first-release purpose/u);
  assert.throws(() => buildRetailMonetaryMovementV1InstructionJson({ ...base, retailAccount: null }), /must be present only/u);
  assert.throws(() => buildRetailMonetaryMovementV1InstructionJson({ ...base, operationDigest: digest(0) }), /must be nonzero/u);
  assert.throws(() => buildRetailMonetaryMovementV1InstructionJson({ ...base, amount: "01.25" }), /canonical quantity/u);
});

test("retail wire schemas and native decoding preserve exact integer values", () => {
  const json = `{"ActivateRetailDailyLimitV1":{"policy":{"physical_dataspace":${DATA_SPACE_ID}}}}`;
  let encodedJson;
  const native = createNativeRuntime({
    noritoDecodeInstruction: () => json,
    noritoDecodeInstructionBoxArchive: () => json,
    noritoEncodeInstruction: (source) => {
      encodedJson = source;
      return Buffer.from([1]);
    },
  });
  const api = _createNoritoInstructionApi(native);
  const bindings = api._instructionWireSchemaBindings().filter(({ outerWireId }) =>
    outerWireId.startsWith("iroha.asset.retail_day."));
  assert.deepEqual(bindings, [
    {
      outerWireId: "iroha.asset.retail_day.activate.v1",
      innerTypeName: "iroha_data_model::isi::retail_daily_limit::ActivateRetailDailyLimitV1",
    },
    {
      outerWireId: "iroha.asset.retail_day.identity.bind.v1",
      innerTypeName: "iroha_data_model::isi::retail_daily_limit::BindRetailIdentityV1",
    },
    {
      outerWireId: "iroha.asset.retail_day.monetary_movement.v1",
      innerTypeName: "iroha_data_model::isi::retail_daily_limit::RetailMonetaryMovementV1",
    },
  ]);
  const decoded = api.noritoDecodeInstruction(Buffer.of(1), 753);
  assert.equal(decoded.ActivateRetailDailyLimitV1.policy.physical_dataspace,
    8648377547929788715n);
  assert.equal(api.noritoDecodeInstructionBoxArchive(Buffer.of(1), 753).ActivateRetailDailyLimitV1.policy.physical_dataspace,
    8648377547929788715n);
  assert.equal(api.noritoDecodeInstruction(Buffer.of(1), 753, { parseJson: false }), json);
  assert.deepEqual(api.noritoEncodeInstruction(decoded, 753), Buffer.of(1));
  assert.equal(encodedJson, json);
});
