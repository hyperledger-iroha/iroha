import { test as baseTest } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  noritoEncodeInstruction,
  noritoDecodeInstruction,
  noritoEncodeMultisigProposeRequest,
  noritoEncodeMultisigContractCallProposeRequest,
  noritoEncodeMultisigContractCallApproveRequest,
} from "../src/norito.js";
import {
  makeNativeTest,
  nativeBinding,
  noritoRequiredMethods,
} from "./helpers/native.js";

const test = makeNativeTest(baseTest, { require: noritoRequiredMethods });
const UNAVAILABLE_NATIVE_BINDING = Object.freeze({
  noritoEncodeInstruction() {
    throw new Error("Native binding required; test override is unavailable");
  },
  noritoDecodeInstruction() {
    throw new Error("Native binding required; test override is unavailable");
  },
});
const ACCOUNT_ID = "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB";
const MULTISIG_SIGNER_ID =
  "sorauﾛ1P738ｷﾈｹｵﾙﾍﾉﾂUｿﾚｹﾑbﾄ1xYﾆｷvWzﾒkﾒ5ﾛﾘuE1ﾌsﾛXB6V1Y";

function canonicalSignatureBase64Fixture() {
  return Buffer.alloc(64, 0x01).toString("base64");
}

function authorityFeePayment(gasLimit = null) {
  return {
    payer: "authority",
    value: { charge_limits: [], gas_limit: gasLimit },
  };
}

function noncanonicalStandardBase64PadBitAlias(encoded) {
  assert.equal(encoded.endsWith("=="), true);
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  const chars = [...encoded];
  const index = chars.length - 3;
  const value = alphabet.indexOf(chars[index]);
  assert.notEqual(value, -1);
  chars[index] = alphabet[value ^ 0x01];
  return chars.join("");
}

function testCrc64Ecma(payload) {
  const mask = 0xffff_ffff_ffff_ffffn;
  const polynomial = 0xc96c_5795_d787_0f42n;
  let crc = mask;
  for (const byte of payload) {
    crc ^= BigInt(byte);
    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc & 1n) === 0n ? crc >> 1n : (crc >> 1n) ^ polynomial;
    }
  }
  return BigInt.asUintN(64, crc ^ mask);
}

function rewriteNestedInstructionFrameCrcs(buffer) {
  const outerPayloadLength = Number(buffer.readBigUInt64LE(23));
  const outerPayloadStart = buffer.length - outerPayloadLength;
  let cursor = outerPayloadStart;
  const wireIdLength = Number(buffer.readBigUInt64LE(cursor));
  cursor += 8 + wireIdLength;
  const innerWrapperLength = Number(buffer.readBigUInt64LE(cursor));
  const innerWrapperStart = cursor + 8;
  assert.equal(innerWrapperStart + innerWrapperLength, buffer.length);
  const innerFrameLength = Number(buffer.readBigUInt64LE(innerWrapperStart));
  const innerFrameStart = innerWrapperStart + 8;
  assert.equal(innerFrameStart + innerFrameLength, buffer.length);
  const innerPayloadLength = Number(buffer.readBigUInt64LE(innerFrameStart + 23));
  const innerPayloadStart = innerFrameStart + innerFrameLength - innerPayloadLength;
  buffer.writeBigUInt64LE(
    testCrc64Ecma(
      buffer.subarray(innerPayloadStart, innerPayloadStart + innerPayloadLength),
    ),
    innerFrameStart + 31,
  );
  buffer.writeBigUInt64LE(
    testCrc64Ecma(buffer.subarray(outerPayloadStart)),
    31,
  );
}

const REGISTER_DOMAIN = {
  Register: {
    Domain: {
      id: "wonderland.sora",
      logo: null,
      metadata: {
        key: "value",
      },
    },
  },
};

const REGISTER_ACCOUNT = {
  Register: {
    Account: {
      id: ACCOUNT_ID,
      label: null,
      uaid: null,
      opaque_ids: [],
      metadata: { nickname: "alice" },
    },
  },
};

const REGISTER_ASSET = {
  Register: {
    AssetDefinition: {
      id: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
      name: "",
      description: null,
      alias: null,
      logo: null,
      metadata: {},
      mintable: "Infinitely",
      spec: { scale: null },
      balance_scope_policy: "Global",
      confidential_policy: {
        mode: "TransparentOnly",
        vk_set_hash: null,
        poseidon_params_id: null,
        pedersen_params_id: null,
        pending_transition: null,
      },
    },
  },
};

const REGISTER_ASSET_WITH_POLICY = {
  Register: {
    AssetDefinition: {
      id: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
      name: "demo",
      description: "Demo settlement PoC asset",
      alias: "demo#settlement.main",
      logo: "sorafs://bafybeigdyrztk/logo/demo.png",
      metadata: {
        purpose: "poc",
      },
      mintable: "Limited(5)",
      spec: { scale: 2 },
      balance_scope_policy: "DataspaceRestricted",
      confidential_policy: {
        mode: "Convertible",
        vk_set_hash:
          "hash:1111111111111111111111111111111111111111111111111111111111111111#4667",
        poseidon_params_id: 7,
        pedersen_params_id: 3,
        pending_transition: {
          new_mode: "ShieldedOnly",
          effective_height: 64,
          previous_mode: "Convertible",
          transition_id:
            "hash:5555555555555555555555555555555555555555555555555555555555555555#2B05",
          conversion_window: 5,
        },
      },
    },
  },
};

const REGISTER_ASSET_HIDDEN_POOL = {
  zk: {
    RegisterAssetHiddenZkPool: {
      pool_id: "boi-private-is-pool",
      storage_asset: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
      asset_set_root: Array.from({ length: 32 }, (_, index) => index + 1),
      vk_transfer: {
        backend: "halo2/ipa",
        name: "asset-hidden-transfer-v1",
      },
    },
  },
};

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..");

function loadInstructionFixture(name) {
  const fixturePath = path.join(repoRoot, "fixtures", "norito_instructions", name);
  const raw = fs.readFileSync(fixturePath, "utf8");
  return JSON.parse(raw);
}

function loadInstructionBytes(name) {
  const fixture = loadInstructionFixture(name);
  return Buffer.from(fixture.instruction, "base64");
}

function loadAssetIdFromFixture(name) {
  const decoded = noritoDecodeInstruction(loadInstructionBytes(name));
  const destination = decoded?.Mint?.Asset?.destination ?? decoded?.Burn?.Asset?.destination;
  if (typeof destination !== "string" || !destination.includes("#")) {
    throw new Error(`fixture ${name} did not decode to canonical public AssetId literal`);
  }
  return destination;
}

function withMissingNativeBinding(callback) {
  const previous = globalThis.__IROHA_NORITO_BINDING__;
  globalThis.__IROHA_NORITO_BINDING__ = UNAVAILABLE_NATIVE_BINDING;
  try {
    return callback();
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NORITO_BINDING__;
    } else {
      globalThis.__IROHA_NORITO_BINDING__ = previous;
    }
  }
}

function readU64Length(buffer, offset, label) {
  assert.ok(offset + 8 <= buffer.length, `${label} length prefix is in bounds`);
  const value = buffer.readBigUInt64LE(offset);
  assert.ok(value <= BigInt(Number.MAX_SAFE_INTEGER), `${label} length fits JS number`);
  return { length: Number(value), bytes: 8 };
}

function readCompactLength(buffer, offset, label) {
  let value = 0n;
  let shift = 0n;
  let cursor = offset;
  for (; cursor < buffer.length; cursor += 1) {
    const byte = buffer[cursor];
    value |= BigInt(byte & 0x7f) << shift;
    if ((byte & 0x80) === 0) {
      assert.ok(value <= BigInt(Number.MAX_SAFE_INTEGER), `${label} length fits JS number`);
      return { length: Number(value), bytes: cursor + 1 - offset };
    }
    shift += 7n;
  }
  assert.fail(`${label} compact length prefix is unterminated`);
}

function readNoritoFieldPayload(buffer, offset, label, compactLength) {
  const { length, bytes } = compactLength
    ? readCompactLength(buffer, offset, label)
    : readU64Length(buffer, offset, label);
  const start = offset + bytes;
  const end = start + length;
  assert.ok(end <= buffer.length, `${label} payload is in bounds`);
  return { payload: buffer.subarray(start, end), offset: end };
}

function noritoFramePayload(body, label) {
  const buffer = Buffer.from(body);
  assert.equal(buffer.subarray(0, 4).toString("ascii"), "NRT0");
  const { length: payloadLength } = readU64Length(buffer, 23, `${label}.payloadLength`);
  assert.equal(buffer.length, 40 + payloadLength);
  return {
    flags: buffer[39],
    payload: buffer.subarray(40),
  };
}

test("noritoEncodeInstruction returns canonical bytes", () => {
  const encoded = noritoEncodeInstruction(REGISTER_DOMAIN);
  assert.ok(Buffer.isBuffer(encoded));
  assert.ok(encoded.length > 32);
});

test("noritoDecodeInstruction round-trips instruction JSON", () => {
  const encoded = noritoEncodeInstruction(REGISTER_DOMAIN);
  const decoded = noritoDecodeInstruction(encoded);
  assert.deepEqual(decoded, REGISTER_DOMAIN);
});

test("norito encode/decode supports account registration", () => {
  const encoded = noritoEncodeInstruction(REGISTER_ACCOUNT);
  const decoded = noritoDecodeInstruction(encoded);
  assert.deepEqual(decoded, REGISTER_ACCOUNT);
});

test("norito encode/decode supports asset definition registration", () => {
  const encoded = noritoEncodeInstruction(REGISTER_ASSET);
  const decoded = noritoDecodeInstruction(encoded);
  assert.deepEqual(decoded, REGISTER_ASSET);
});

baseTest("pure JS Norito codec supports asset definition registration without native binding", () => {
  withMissingNativeBinding(() => {
    const encoded = noritoEncodeInstruction(REGISTER_ASSET);
    const decoded = noritoDecodeInstruction(encoded);
    assert.deepEqual(decoded, REGISTER_ASSET);
  });
});

baseTest("pure JS Norito asset definition codec preserves policy fields", () => {
  withMissingNativeBinding(() => {
    const encoded = noritoEncodeInstruction(REGISTER_ASSET_WITH_POLICY);
    const decoded = noritoDecodeInstruction(encoded);
    assert.deepEqual(decoded, REGISTER_ASSET_WITH_POLICY);
  });
});

test("native Norito decoder accepts pure JS asset definition frames", () => {
  const encoded = withMissingNativeBinding(() =>
    noritoEncodeInstruction(REGISTER_ASSET_WITH_POLICY),
  );
  assert.deepEqual(noritoDecodeInstruction(encoded), REGISTER_ASSET_WITH_POLICY);
});

baseTest("pure JS Norito codec supports asset-hidden pool registration without native binding", () => {
  withMissingNativeBinding(() => {
    const encoded = noritoEncodeInstruction(REGISTER_ASSET_HIDDEN_POOL);
    const decoded = noritoDecodeInstruction(encoded);
    assert.deepEqual(decoded, REGISTER_ASSET_HIDDEN_POOL);
  });
});

test("unsupported instruction fallback cannot change later native encodings", () => {
  const supported = noritoDecodeInstruction(
    loadInstructionBytes("burn_asset_quantity.json"),
  );
  const expected = Buffer.from(
    nativeBinding.noritoEncodeInstruction(JSON.stringify(supported)),
  );

  const fallback = Buffer.from(
    noritoEncodeInstruction(REGISTER_ASSET_HIDDEN_POOL),
  );
  assert.ok(fallback.length > 32);

  assert.deepEqual(
    Buffer.from(noritoEncodeInstruction(supported)),
    expected,
    "an automatic fallback for one unsupported instruction must remain per-call",
  );
});

test("native Norito decoder accepts pure JS asset-hidden pool registration frames", () => {
  const encoded = withMissingNativeBinding(() =>
    noritoEncodeInstruction(REGISTER_ASSET_HIDDEN_POOL),
  );
  assert.deepEqual(noritoDecodeInstruction(encoded), REGISTER_ASSET_HIDDEN_POOL);
});

baseTest("pure JS Norito asset definition codec rejects adversarial fields", () => {
  const withAssetPatch = (patch) => ({
    Register: {
      AssetDefinition: {
        ...REGISTER_ASSET.Register.AssetDefinition,
        ...patch,
      },
    },
  });
  withMissingNativeBinding(() => {
    assert.throws(
      () => noritoEncodeInstruction(withAssetPatch({ mintable: "Limited(0)" })),
      /positive unsigned 32-bit integer/,
    );
    assert.throws(
      () =>
        noritoEncodeInstruction(
          withAssetPatch({ balance_scope_policy: "ObserverScoped" }),
        ),
      /Global or DataspaceRestricted/,
    );
    assert.throws(
      () =>
        noritoEncodeInstruction(
          withAssetPatch({
            confidential_policy: {
              ...REGISTER_ASSET.Register.AssetDefinition.confidential_policy,
              mode: "Mixed",
            },
          }),
        ),
      /TransparentOnly, ShieldedOnly, or Convertible/,
    );
    assert.throws(
      () => noritoEncodeInstruction(withAssetPatch({ logo: "https://example.invalid/logo.png" })),
      /sorafs:\/\/ URI/,
    );
    assert.throws(
      () =>
        noritoEncodeInstruction(
          withAssetPatch({ id: "62Fk4FPcMuLvW5QjDGNF2a4jAmxM" }),
        ),
      /checksum is invalid/,
    );
  });
});

test("norito encode/decode supports mint asset instructions", () => {
  const instruction = {
    Mint: {
      Asset: {
        object: "42",
        destination: loadAssetIdFromFixture("mint_asset_quantity.json"),
      },
    },
  };
  const encoded = noritoEncodeInstruction(instruction);
  const decoded = noritoDecodeInstruction(encoded);
  assert.deepEqual(decoded, instruction);
});

test("norito encode/decode supports transfer asset instructions", () => {
  const instruction = {
    Transfer: {
      Asset: {
        source: loadAssetIdFromFixture("mint_asset_quantity.json"),
        object: "10",
        destination: ACCOUNT_ID,
      },
    },
  };
  const encoded = noritoEncodeInstruction(instruction);
  const decoded = noritoDecodeInstruction(encoded);
  assert.deepEqual(decoded, instruction);
});

baseTest("noritoEncodeInstruction uses the pure JS codec for supported instruction JSON", () => {
  const instruction = {
    Transfer: {
      Asset: {
        source: loadAssetIdFromFixture("mint_asset_quantity.json"),
        object: "7",
        destination: ACCOUNT_ID,
      },
    },
  };
  let encoded;
  withMissingNativeBinding(() => {
    encoded = Buffer.from(noritoEncodeInstruction(instruction));
  });
  assert.ok(encoded.length > 32);
  assert.deepEqual(noritoDecodeInstruction(encoded), instruction);

});

baseTest("contract manifest codec preserves the canonical seiyaku name", () => {
  const instruction = {
    RegisterSmartContractCode: {
      manifest: {
        seiyaku_name: "Ledger",
        entrypoints: null,
        kotoba: null,
      },
    },
  };
  let encoded;
  withMissingNativeBinding(() => {
    encoded = Buffer.from(noritoEncodeInstruction(instruction));
  });
  assert.deepEqual(noritoDecodeInstruction(encoded), {
    RegisterSmartContractCode: {
      manifest: {
        seiyaku_name: "Ledger",
        code_hash: null,
        abi_hash: null,
        compiler_fingerprint: null,
        features_bitmap: null,
        access_set_hints: null,
        entrypoints: null,
        states: null,
        error_codes: null,
        kotoba: null,
        provenance: null,
      },
    },
  });
});

baseTest("contract manifest codec matches Rust V1 trigger bytes", () => {
  const fixture = JSON.parse(
    fs.readFileSync(path.join(__dirname, "fixtures", "contract_manifest_v1.json"), "utf8"),
  );
  const instruction = {
    RegisterSmartContractCode: { manifest: fixture.manifest },
  };
  const encoded = withMissingNativeBinding(() =>
    Buffer.from(noritoEncodeInstruction(instruction)),
  );
  const rustManifest = Buffer.from(fixture.manifest_compact_hex, "hex");
  assert.notEqual(
    encoded.indexOf(rustManifest),
    -1,
    "instruction embeds the exact Rust-generated compact manifest payload",
  );
  assert.deepEqual(noritoDecodeInstruction(encoded), instruction);

  const signedInstruction = {
    RegisterSmartContractCode: {
      manifest: {
        ...fixture.manifest,
        provenance: fixture.signed_provenance,
      },
    },
  };
  const signedEncoded = withMissingNativeBinding(() =>
    Buffer.from(noritoEncodeInstruction(signedInstruction)),
  );
  assert.notEqual(
    signedEncoded.indexOf(Buffer.from(fixture.signed_manifest_compact_hex, "hex")),
    -1,
    "provenance uses the exact Rust PublicKey and Signature wire layout",
  );
  assert.deepEqual(noritoDecodeInstruction(signedEncoded), signedInstruction);
});

baseTest("contract manifest codec roundtrips every V1 descriptor field", () => {
  const filterFixture = JSON.parse(
    fs.readFileSync(path.join(__dirname, "fixtures", "contract_manifest_v1.json"), "utf8"),
  ).manifest.entrypoints[0].triggers[0].filter;
  const leaf = (kind) => ({
    nodes: [{ kind: "Leaf", value: { kind, value: null } }],
  });
  const manifest = {
    seiyaku_name: "Ledger",
    code_hash: "aa".repeat(32),
    abi_hash: "bb".repeat(32),
    compiler_fingerprint: "kotodama_lang",
    features_bitmap: 42,
    access_set_hints: {
      read_keys: ["state:Balances"],
      write_keys: ["state:Balances"],
      dynamic_reads: [
        {
          base_key: "state:Balances",
          key_type: "AccountId",
          bound_kind: "take",
          max_keys: 4,
        },
      ],
      dynamic_writes: [],
    },
    entrypoints: [
      {
        name: "transfer",
        kind: { kind: "Kotoage", value: null },
        params: [
          { name: "amount", type_name: "quantity" },
          { name: "tags", type_name: "List<Name, 64>" },
        ],
        argument_schema: {
          fields: [
            { name: "amount", ty: leaf("Quantity") },
            {
              name: "tags",
              ty: {
                nodes: [
                  { kind: "List", value: { capacity: 64 } },
                  { kind: "Leaf", value: { kind: "Name", value: null } },
                ],
              },
            },
          ],
        },
        return_type: "Result<bool, string>",
        return_schema: {
          nodes: [
            { kind: "Result", value: null },
            { kind: "Leaf", value: { kind: "Bool", value: null } },
            { kind: "Leaf", value: { kind: "String", value: null } },
          ],
        },
        permission: "TransferAsset",
        read_keys: ["state:Balances"],
        write_keys: ["state:Balances"],
        access_hints_complete: true,
        access_hints_skipped: [],
        triggers: [
          {
            id: "settle",
            repeats: { Exactly: 3 },
            filter: filterFixture,
            authority: null,
            metadata: { priority: 7 },
            callback: { namespace: null, entrypoint: "transfer" },
          },
        ],
      },
    ],
    states: [{ name: "Balances", type_name: "StateMap<AccountId, quantity>" }],
    error_codes: [{ namespace: "LedgerError", name: "Denied", code: 7 }],
    kotoba: [
      {
        msg_id: "ledger.denied",
        translations: [
          { lang: "en", text: "" },
          { lang: "ja", text: "拒否" },
        ],
      },
    ],
    provenance: {
      signer: `ed0120${"11".repeat(32)}`,
      signature: "22".repeat(64).toUpperCase(),
    },
  };
  const encoded = withMissingNativeBinding(() =>
    Buffer.from(
      noritoEncodeInstruction({ RegisterSmartContractCode: { manifest } }),
    ),
  );
  const decoded = noritoDecodeInstruction(encoded);
  assert.deepEqual(decoded.RegisterSmartContractCode.manifest, {
    ...manifest,
    code_hash: "hash:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA#0E5B",
    abi_hash: "hash:BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB#ABA2",
  });
});

baseTest("contract manifest codec rejects noncanonical and retired layouts", () => {
  const fixture = JSON.parse(
    fs.readFileSync(path.join(__dirname, "fixtures", "contract_manifest_v1.json"), "utf8"),
  );
  const encodeManifest = (manifest) =>
    withMissingNativeBinding(() =>
      noritoEncodeInstruction({ RegisterSmartContractCode: { manifest } }),
    );

  assert.throws(
    () => encodeManifest({ ...fixture.manifest, contract_name: "Legacy" }),
    /unknown field contract_name/u,
  );
  assert.throws(
    () =>
      encodeManifest({
        ...fixture.manifest,
        entrypoints: [
          { ...fixture.manifest.entrypoints[0], kind: { kind: "Public", value: null } },
        ],
      }),
    /Kotoage, View, Hajimari, or Kaizen/u,
  );
  assert.throws(
    () =>
      encodeManifest({
        ...fixture.manifest,
        entrypoints: [
          {
            ...fixture.manifest.entrypoints[0],
            argument_schema: {
              fields: [
                {
                  name: "bad",
                  ty: {
                    nodes: [
                      { kind: "Leaf", value: { kind: "Opaque", value: null } },
                    ],
                  },
                },
              ],
            },
          },
        ],
      }),
    /unsupported value kind Opaque|not a canonical V1 entrypoint value kind/u,
  );
  assert.throws(
    () =>
      encodeManifest({
        ...fixture.manifest,
        entrypoints: [
          {
            ...fixture.manifest.entrypoints[0],
            argument_schema: {
              fields: [
                {
                  name: "legacy_list",
                  ty: {
                    nodes: [
                      {
                        kind: "List",
                        value: {
                          element: {
                            nodes: [
                              {
                                kind: "Leaf",
                                value: { kind: "Name", value: null },
                              },
                            ],
                          },
                          capacity: 64,
                        },
                      },
                    ],
                  },
                },
              ],
            },
          },
        ],
      }),
    /unknown field element|contain (?:only|exactly) capacity/u,
  );

  const badFilter = Buffer.from(
    fixture.event_filter_box.norito_frame_hex,
    "hex",
  );
  badFilter[6] ^= 0x01;
  assert.throws(
    () =>
      encodeManifest({
        ...fixture.manifest,
        entrypoints: [
          {
            ...fixture.manifest.entrypoints[0],
            triggers: [
              {
                ...fixture.manifest.entrypoints[0].triggers[0],
                filter: badFilter.toString("base64"),
              },
            ],
          },
        ],
      }),
    /schema hash did not match/u,
  );
  assert.throws(
    () =>
      encodeManifest({
        ...fixture.manifest,
        provenance: {
          signer: `ed0120${"11".repeat(32)}`,
          signature: "00".repeat(64),
        },
      }),
    /must not be all zero/u,
  );
});

baseTest("contract manifest codec validates every flat query schema and ordinary structs", () => {
  const fixture = JSON.parse(
    fs.readFileSync(path.join(__dirname, "fixtures", "contract_manifest_v1.json"), "utf8"),
  );
  const leaf = (kind) => ({ kind: "Leaf", value: { kind, value: null } });
  const layouts = [
    ["AccountView", ["id", "metadata"], [leaf("AccountId"), leaf("Json")]],
    ["AssetView", ["id", "amount"], [leaf("AssetId"), leaf("Quantity")]],
    [
      "AssetDefinitionView",
      ["id", "name", "description", "owned_by", "total_quantity", "metadata"],
      [
        leaf("AssetDefinitionId"),
        leaf("String"),
        { kind: "Option", value: null },
        leaf("String"),
        leaf("AccountId"),
        leaf("Quantity"),
        leaf("Json"),
      ],
    ],
    [
      "DomainView",
      ["id", "owned_by", "metadata"],
      [leaf("DomainId"), leaf("AccountId"), leaf("Json")],
    ],
    [
      "NftView",
      ["id", "owned_by", "content"],
      [leaf("NftId"), leaf("AccountId"), leaf("Json")],
    ],
  ];
  const instruction = (returnType, nodes) => ({
    RegisterSmartContractCode: {
      manifest: {
        ...fixture.manifest,
        entrypoints: [
          {
            ...fixture.manifest.entrypoints[0],
            name: "read",
            kind: { kind: "View", value: null },
            return_type: returnType,
            return_schema: { nodes },
            permission: null,
            triggers: [],
          },
        ],
      },
    },
  });
  const roundtrip = (returnType, nodes) => {
    const value = instruction(returnType, nodes);
    const encoded = withMissingNativeBinding(() => noritoEncodeInstruction(value));
    assert.deepEqual(noritoDecodeInstruction(encoded), value);
  };

  for (const [name, fields, children] of layouts) {
    const view = [{ kind: "Struct", value: { name, fields } }, ...children];
    roundtrip(name, view);
    roundtrip(`Option<${name}>`, [{ kind: "Option", value: null }, ...view]);
    roundtrip(`QueryPage<${name}>`, [
      {
        kind: "Struct",
        value: { name: "QueryPage", fields: ["items", "next_offset"] },
      },
      { kind: "List", value: { capacity: 64 } },
      ...view,
      { kind: "Option", value: null },
      leaf("Int"),
    ]);
  }
  roundtrip("struct Pair", [
    { kind: "Struct", value: { name: "Pair", fields: ["left", "right"] } },
    leaf("Int"),
    leaf("Bool"),
  ]);
});

baseTest("contract manifest codec rejects malformed and forged flat schema tapes", () => {
  const fixture = JSON.parse(
    fs.readFileSync(path.join(__dirname, "fixtures", "contract_manifest_v1.json"), "utf8"),
  );
  const leaf = (kind) => ({ kind: "Leaf", value: { kind, value: null } });
  const encodeNodes = (nodes) =>
    withMissingNativeBinding(() =>
      noritoEncodeInstruction({
        RegisterSmartContractCode: {
          manifest: {
            ...fixture.manifest,
            entrypoints: [
              {
                ...fixture.manifest.entrypoints[0],
                name: "read",
                kind: { kind: "View", value: null },
                return_type: "schema-under-test",
                return_schema: { nodes },
                permission: null,
                triggers: [],
              },
            ],
          },
        },
      }),
    );

  for (const malformed of [
    [],
    [{ kind: "List", value: { capacity: 1 } }],
    [leaf("Int"), leaf("Bool")],
    [
      { kind: "List", value: { capacity: 1, element: { nodes: [leaf("Int")] } } },
      leaf("Int"),
    ],
    [{ kind: "List", value: { capacity: 0 } }, leaf("Int")],
    [{ kind: "List", value: { capacity: 65 } }, leaf("Int")],
    [
      ...Array.from({ length: 256 }, () => ({
        kind: "List",
        value: { capacity: 1 },
      })),
      leaf("Int"),
    ],
  ]) {
    assert.throws(() => encodeNodes(malformed), /canonical|capacity|complete|exactly capacity/u);
  }

  const reservedViews = [
    ["AccountView", ["id", "metadata"], [leaf("AccountId"), leaf("Json")]],
    ["AssetView", ["id", "amount"], [leaf("AssetId"), leaf("Quantity")]],
    [
      "AssetDefinitionView",
      ["id", "name", "description", "owned_by", "total_quantity", "metadata"],
      [
        leaf("AssetDefinitionId"),
        leaf("String"),
        { kind: "Option", value: null },
        leaf("String"),
        leaf("AccountId"),
        leaf("Quantity"),
        leaf("Json"),
      ],
    ],
    [
      "DomainView",
      ["id", "owned_by", "metadata"],
      [leaf("DomainId"), leaf("AccountId"), leaf("Json")],
    ],
    [
      "NftView",
      ["id", "owned_by", "content"],
      [leaf("NftId"), leaf("AccountId"), leaf("Json")],
    ],
  ];
  for (const [name, fields, children] of reservedViews) {
    const forged = [
      { kind: "Struct", value: { name, fields } },
      ...structuredClone(children),
    ];
    forged[1].value.kind = "Bool";
    assert.throws(() => encodeNodes(forged), /forged reserved query-view/u);

    const forgedPage = [
      {
        kind: "Struct",
        value: { name: "QueryPage", fields: ["items", "next_offset"] },
      },
      { kind: "List", value: { capacity: 32 } },
      { kind: "Struct", value: { name, fields } },
      ...children,
      { kind: "Option", value: null },
      leaf("Int"),
    ];
    assert.throws(() => encodeNodes(forgedPage), /forged QueryPage/u);
  }

  const validCapacity = Buffer.from(
    encodeNodes([{ kind: "List", value: { capacity: 63 } }, leaf("Int")]),
  );
  const comparisonCapacity = Buffer.from(
    encodeNodes([{ kind: "List", value: { capacity: 62 } }, leaf("Int")]),
  );
  const capacityOffsets = Array.from(validCapacity.keys()).filter(
    (index) => validCapacity[index] === 63 && comparisonCapacity[index] === 62,
  );
  assert.equal(capacityOffsets.length, 1);
  for (const invalidCapacity of [0, 65]) {
    const forged = Buffer.from(validCapacity);
    forged[capacityOffsets[0]] = invalidCapacity;
    rewriteNestedInstructionFrameCrcs(forged);
    assert.throws(
      () => withMissingNativeBinding(() => noritoDecodeInstruction(forged)),
      /capacity.*1\.\.64/u,
    );
  }

  const forgedViewWire = Buffer.from(
    encodeNodes([
      {
        kind: "Struct",
        value: { name: "AccountViex", fields: ["id", "metadata"] },
      },
      leaf("Bool"),
      leaf("Json"),
    ]),
  );
  const forgedName = Buffer.from("AccountViex", "utf8");
  const nameOffset = forgedViewWire.indexOf(forgedName);
  assert.notEqual(nameOffset, -1);
  forgedViewWire[nameOffset + forgedName.length - 1] = "w".charCodeAt(0);
  rewriteNestedInstructionFrameCrcs(forgedViewWire);
  assert.throws(
    () => withMissingNativeBinding(() => noritoDecodeInstruction(forgedViewWire)),
    /forged reserved query-view/u,
  );
});

baseTest("native multisig proposal DTO embeds pure JS instructions with compact inner frames", () => {
  const sourceAssetId = loadAssetIdFromFixture("mint_asset_quantity.json");
  const instruction = {
    Transfer: {
      Asset: {
        source: sourceAssetId,
        object: "7",
        destination: MULTISIG_SIGNER_ID,
      },
    },
  };
  const request = {
    multisig_account_alias: "cbdc@hbl.sbp",
    signer_account_id: MULTISIG_SIGNER_ID,
    fee_payment: authorityFeePayment(),
    validation_fee_policy_version: "7",
    validation_fee_policy_hash: "ab".repeat(32),
    validation_fee_instruction_index: "1",
    validation_fee_transfer_entry_index: "2",
    instructions: [instruction],
  };
  const nativeBody = Buffer.from(noritoEncodeMultisigProposeRequest(request));
  const body = withMissingNativeBinding(() =>
    Buffer.from(noritoEncodeMultisigProposeRequest(request)),
  );
  assert.deepEqual(body, nativeBody);

  const outer = noritoFramePayload(body, "MultisigProposeDto");
  const outerUsesCompactLengths = (outer.flags & 0x02) !== 0;
  assert.equal(outerUsesCompactLengths, true);
  let offset = 0;
  for (const fieldName of [
    "multisig_account_id",
    "multisig_account_alias",
    "signer_account_id",
    "private_key",
    "public_key_hex",
    "signature_b64",
    "creation_time_ms",
    "fee_payment",
    "memo",
    "validation_fee_policy_version",
    "validation_fee_policy_hash",
  ]) {
    offset = readNoritoFieldPayload(
      outer.payload,
      offset,
      `MultisigProposeDto.${fieldName}`,
      outerUsesCompactLengths,
    ).offset;
  }
  const instructions = readNoritoFieldPayload(
    outer.payload,
    offset,
    "MultisigProposeDto.instructions",
    outerUsesCompactLengths,
  );
  const count = readU64Length(instructions.payload, 0, "MultisigProposeDto.instructions.count");
  assert.equal(count.length, 1);
  const firstInstruction = readNoritoFieldPayload(
    instructions.payload,
    count.bytes,
    "MultisigProposeDto.instructions[0]",
    outerUsesCompactLengths,
  );
  const wireId = readNoritoFieldPayload(
    firstInstruction.payload,
    0,
    "MultisigProposeDto.instructions[0].wire_id",
    outerUsesCompactLengths,
  );
  const wireIdValue = readNoritoFieldPayload(
    wireId.payload,
    0,
    "MultisigProposeDto.instructions[0].wire_id.value",
    outerUsesCompactLengths,
  );
  assert.equal(wireIdValue.payload.toString("utf8"), "iroha.transfer");
  const embeddedFrameField = readNoritoFieldPayload(
    firstInstruction.payload,
    wireId.offset,
    "MultisigProposeDto.instructions[0].payload",
    outerUsesCompactLengths,
  );
  const embeddedFrame = readNoritoFieldPayload(
    embeddedFrameField.payload,
    0,
    "MultisigProposeDto.instructions[0].payload.frame",
    false,
  );
  const inner = noritoFramePayload(
    embeddedFrame.payload,
    "MultisigProposeDto.instructions[0].payload.frame",
  );
  assert.equal((inner.flags & 0x02) !== 0, true);
  const feeInstructionIndex = readNoritoFieldPayload(
    outer.payload,
    instructions.offset,
    "MultisigProposeDto.validation_fee_instruction_index",
    outerUsesCompactLengths,
  );
  assert.equal(feeInstructionIndex.payload[0], 1);
  const feeInstructionIndexValue = readNoritoFieldPayload(
    feeInstructionIndex.payload,
    1,
    "MultisigProposeDto.validation_fee_instruction_index.value",
    outerUsesCompactLengths,
  );
  const feeInstructionIndexString = readNoritoFieldPayload(
    feeInstructionIndexValue.payload,
    0,
    "MultisigProposeDto.validation_fee_instruction_index.value.string",
    outerUsesCompactLengths,
  );
  assert.equal(feeInstructionIndexString.payload.toString("utf8"), "1");
  assert.equal(feeInstructionIndexString.offset, feeInstructionIndexValue.payload.length);
  assert.equal(feeInstructionIndexValue.offset, feeInstructionIndex.payload.length);
  const feeTransferEntryIndex = readNoritoFieldPayload(
    outer.payload,
    feeInstructionIndex.offset,
    "MultisigProposeDto.validation_fee_transfer_entry_index",
    outerUsesCompactLengths,
  );
  assert.equal(feeTransferEntryIndex.offset, outer.payload.length);
  assert.equal(feeTransferEntryIndex.payload[0], 1);
  const feeTransferEntryIndexValue = readNoritoFieldPayload(
    feeTransferEntryIndex.payload,
    1,
    "MultisigProposeDto.validation_fee_transfer_entry_index.value",
    outerUsesCompactLengths,
  );
  const feeTransferEntryIndexString = readNoritoFieldPayload(
    feeTransferEntryIndexValue.payload,
    0,
    "MultisigProposeDto.validation_fee_transfer_entry_index.value.string",
    outerUsesCompactLengths,
  );
  assert.equal(feeTransferEntryIndexString.payload.toString("utf8"), "2");
  assert.equal(feeTransferEntryIndexString.offset, feeTransferEntryIndexValue.payload.length);
  assert.equal(feeTransferEntryIndexValue.offset, feeTransferEntryIndex.payload.length);
});

test("native multisig proposal DTO rejects malformed validation-fee metadata", () => {
  const request = {
    multisig_account_alias: "cbdc@hbl.sbp",
    signer_account_id: MULTISIG_SIGNER_ID,
    fee_payment: authorityFeePayment(),
    instructions: [
      {
        Transfer: {
          Asset: {
            source: loadAssetIdFromFixture("mint_asset_quantity.json"),
            object: "7",
            destination: ACCOUNT_ID,
          },
        },
      },
    ],
  };

  for (const [fieldName, value] of Object.entries({
    validationFeePolicyVersion: "7",
    validationFeePolicyHash: "ab".repeat(32),
    validationFeeInstructionIndex: "1",
    validationFeeTransferEntryIndex: "2",
  })) {
    assert.throws(
      () =>
        noritoEncodeMultisigProposeRequest({
          ...request,
          [fieldName]: value,
        }),
      /unsupported camelCase validation fee field/,
    );
  }

  assert.throws(
    () =>
      noritoEncodeMultisigProposeRequest({
        ...request,
        validation_fee_instruction_index: "1",
      }),
    /requires validation fee policy metadata/,
  );
  assert.throws(
    () =>
      noritoEncodeMultisigProposeRequest({
        ...request,
        validation_fee_transfer_entry_index: "2",
      }),
    /requires validation fee policy metadata/,
  );
  assert.throws(
    () =>
      noritoEncodeMultisigProposeRequest({
        ...request,
        validation_fee_policy_version: "7",
      }),
    /must be provided together/,
  );
  assert.throws(
    () =>
      noritoEncodeMultisigProposeRequest({
        ...request,
        validation_fee_policy_version: "7",
        validation_fee_policy_hash: "ab",
      }),
    /32-byte hex string/,
  );
  assert.throws(
    () =>
      noritoEncodeMultisigProposeRequest({
        ...request,
        validation_fee_policy_version: "7",
        validation_fee_policy_hash: "ab".repeat(32),
        validation_fee_transfer_entry_index: "2",
      }),
    /requires validation_fee_instruction_index/,
  );
  assert.throws(
    () =>
      noritoEncodeMultisigProposeRequest({
        ...request,
        validation_fee_policy_version: "7",
        validation_fee_policy_hash: "ab".repeat(32),
        validation_fee_instruction_index: "-1",
      }),
    /must be a bigint, integer number, or decimal string/,
  );
  assert.throws(
    () =>
      noritoEncodeMultisigProposeRequest({
        ...request,
        validation_fee_policy_version: "7",
        validation_fee_policy_hash: "ab".repeat(32),
        validation_fee_instruction_index: "1",
        validation_fee_transfer_entry_index: "-2",
      }),
    /must be a bigint, integer number, or decimal string/,
  );
});

test("native multisig proposal DTO preserves native instruction frames without JS schema entries", () => {
  const request = {
    multisig_account_alias: "cbdc@hbl.sbp",
    signer_account_id: MULTISIG_SIGNER_ID,
    fee_payment: authorityFeePayment(),
    instructions: [
      {
        Unregister: {
          Domain: "wonderland.sora",
        },
      },
    ],
  };

  const body = Buffer.from(noritoEncodeMultisigProposeRequest(request));
  assert.ok(body.length > 32);
});

baseTest("native multisig DTO encoders reject noncanonical signature_b64 text", () => {
  const canonicalSignature = canonicalSignatureBase64Fixture();
  const invalidSignatures = [
    ` ${canonicalSignature} `,
    canonicalSignature.replace(/=+$/u, ""),
    noncanonicalStandardBase64PadBitAlias(canonicalSignature),
  ];
  for (const signature_b64 of invalidSignatures) {
    assert.throws(
      () =>
        noritoEncodeMultisigProposeRequest({
          multisig_account_alias: "cbdc@hbl.sbp",
          signer_account_id: MULTISIG_SIGNER_ID,
          signature_b64,
          fee_payment: authorityFeePayment(),
          instructions: [
            {
              Unregister: {
                Domain: "wonderland.sora",
              },
            },
          ],
        }),
      /exact standard-base64/,
    );
    assert.throws(
      () =>
        noritoEncodeMultisigContractCallProposeRequest({
          multisig_account_alias: "cbdc@hbl.sbp",
          signer_account_id: MULTISIG_SIGNER_ID,
          signature_b64,
          contract_address: "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
          entrypoint: "execute",
          payload: { probe: true },
          fee_payment: authorityFeePayment(10_000),
        }),
      /exact standard-base64/,
    );
    assert.throws(
      () =>
        noritoEncodeMultisigContractCallApproveRequest({
          multisig_account_alias: "cbdc@hbl.sbp",
          signer_account_id: MULTISIG_SIGNER_ID,
          signature_b64,
          instructions_hash: "aa".repeat(32),
          fee_payment: authorityFeePayment(),
        }),
      /exact standard-base64/,
    );
  }
});

baseTest("noritoDecodeInstruction decodes supported canonical bytes without native binding", () => {
  const bytes = loadInstructionBytes("mint_asset_quantity.json");
  const decoded = withMissingNativeBinding(() => noritoDecodeInstruction(bytes));
  assert.ok(decoded?.Mint?.Asset);
});

baseTest("noritoEncodeInstruction passes pre-encoded payloads through without native binding", () => {
  const payload = Buffer.from([1, 2, 3, 4]);
  withMissingNativeBinding(() => {
    assert.strictEqual(noritoEncodeInstruction(payload), payload);
    assert.deepEqual(noritoEncodeInstruction(payload.toString("base64")), payload);
    assert.deepEqual(noritoEncodeInstruction(`0x${payload.toString("hex")}`), payload);
  });
});

test("norito encode/decode supports ExecuteTrigger instructions", () => {
  const instruction = {
    ExecuteTrigger: {
      trigger: "mint_request_hbl",
      args: {
        action: "create",
        request_id: "mr1",
      },
    },
  };
  const encoded = noritoEncodeInstruction(instruction);
  const decoded = noritoDecodeInstruction(encoded);
  assert.deepEqual(decoded, instruction);
});

test("noritoDecodeInstruction keeps canonical asset-holding ids without @domain rewrites", () => {
  const bytes = loadInstructionBytes("mint_asset_quantity.json");
  const decoded = noritoDecodeInstruction(bytes);
  const assetId = decoded?.Mint?.Asset?.destination;
  assert.equal(typeof assetId, "string");
  assert.equal(assetId.includes("#"), true);
  assert.equal(assetId.includes("@"), false);
});

test("noritoDecodeInstruction preserves nested asset-holding identifiers", () => {
  const bytes = loadInstructionBytes("burn_asset_quantity.json");
  const decoded = noritoDecodeInstruction(bytes);
  const assetId = decoded?.Burn?.Asset?.destination;
  assert.equal(typeof assetId, "string");
  assert.equal(assetId.includes("#"), true);
  assert.equal(assetId.includes("@"), false);
});

test("noritoDecodeInstruction can return raw JSON string", () => {
  const encoded = noritoEncodeInstruction(REGISTER_DOMAIN);
  const json = noritoDecodeInstruction(encoded, { parseJson: false });
  assert.equal(typeof json, "string");
  const parsed = JSON.parse(json);
  assert.deepEqual(parsed, REGISTER_DOMAIN);
});

test("burn asset fixture matches canonical Norito bytes", () => {
  const bytes = loadInstructionBytes("burn_asset_quantity.json");
  const instruction = noritoDecodeInstruction(bytes);
  const expectedHex = bytes.toString("hex");
  assert.equal(typeof expectedHex, "string");
  const encoded = noritoEncodeInstruction(instruction);
  assert.ok(Buffer.isBuffer(encoded));
  const encodedHex = encoded.toString("hex");
  assert.equal(encodedHex, expectedHex);
});

test("burn asset fractional fixture matches canonical Norito bytes", () => {
  const bytes = loadInstructionBytes("burn_asset_fractional.json");
  const instruction = noritoDecodeInstruction(bytes);
  const expectedHex = bytes.toString("hex");
  assert.equal(typeof expectedHex, "string");
  const encoded = noritoEncodeInstruction(instruction);
  assert.ok(Buffer.isBuffer(encoded));
  const encodedHex = encoded.toString("hex");
  assert.equal(encodedHex, expectedHex);
});

test("mint asset fixture matches canonical Norito bytes", () => {
  const bytes = loadInstructionBytes("mint_asset_quantity.json");
  const instruction = noritoDecodeInstruction(bytes);
  const expectedHex = bytes.toString("hex");
  assert.equal(typeof expectedHex, "string");
  const encoded = noritoEncodeInstruction(instruction);
  assert.ok(Buffer.isBuffer(encoded));
  const encodedHex = encoded.toString("hex");
  assert.equal(encodedHex, expectedHex);
});

test("burn trigger fixture matches canonical Norito bytes", () => {
  const bytes = loadInstructionBytes("burn_trigger_repetitions.json");
  const instruction = noritoDecodeInstruction(bytes);
  const expectedHex = bytes.toString("hex");
  assert.equal(typeof expectedHex, "string");
  const encoded = noritoEncodeInstruction(instruction);
  assert.ok(Buffer.isBuffer(encoded));
  const encodedHex = encoded.toString("hex");
  assert.equal(encodedHex, expectedHex);
});

baseTest("noritoEncodeInstruction requires native binding for unsupported instruction JSON", () => {
  const instruction = {
    Log: {
      level: "INFO",
      message: "unsupported by the pure JS fallback",
    },
  };
  withMissingNativeBinding(() => {
    assert.throws(
      () => noritoEncodeInstruction(instruction),
      /Native binding required/,
    );
  });
});
