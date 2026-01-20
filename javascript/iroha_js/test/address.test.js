"use strict";

import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  AccountAddress,
  AccountAddressError,
  AccountAddressErrorCode,
  AccountAddressFormat,
  canonicalizeDomainLabel,
  DEFAULT_DOMAIN_NAME,
  decodeCompressedAccountAddress,
  encodeCompressedAccountAddress,
  inspectAccountId,
  configureCurveSupport,
} from "../src/address.js";

function hexToBytes(hex) {
  const body = hex.replace(/^0x/i, "");
  if (body.length % 2 !== 0) {
    throw new TypeError("hex string must have even length");
  }
  const out = new Uint8Array(body.length / 2);
  for (let index = 0; index < out.length; index += 1) {
    out[index] = parseInt(body.slice(index * 2, index * 2 + 2), 16);
  }
  return out;
}

const DEFAULT_PUBLIC_KEY = hexToBytes(
  "641297079357229F295938A4B5A333DE35069BF47B9D0704E45805713D13C201",
);
const ALT_PUBLIC_KEY = hexToBytes(
  "3B77A042F1DE02F6D5F418F36A20FD68C8329FE3BBFBECD26A2D72878CD827F8",
);
const GOST256_PUBLIC_KEY = hexToBytes(
  "6b167e468ada39cfbdeecb3ac490d130624c68dc8b8454ed836b7b19ae383fef9f116838fb5b1cf548ea5913827d4a0636b64537f05b13d6fecb121eac4fbaea",
);
const SM2_PUBLIC_KEY = hexToBytes(
  "04361255A512347E76EA947EBB416C12D4C07E30B150C0EC2047ECC5E142907499B8D99C4C5CF69BFF6527E7B67396B55E42EF98625B339696DBEF9A3AABBFC06F",
);
const MULTISIG_CANONICAL_HEX =
  "0x0a014b5d81318d0cdf5559258c8e01010003030100010020591b509f4a1b29c9d8cada0f876ee97041cbd43c0de3251b1e6c77bb621f40b0010002002062ffa8de6da654efebfa3d80fdef8f188034cfa3e211859346e41a5fea06532b0100010020d4abafc1f385826a8ebc60df90568c52ccfbf91e0b134dc66e42731a7561e4da";
const MULTISIG_CANONICAL_BYTES = hexToBytes(MULTISIG_CANONICAL_HEX);

test("configureCurveSupport validates options and gating toggles", () => {
  configureCurveSupport();
  try {
    for (const value of [null, [], "allow"]) {
      assert.throws(
        () => configureCurveSupport(value),
        (error) =>
          error instanceof TypeError &&
          /configureCurveSupport options must be an object/.test(error.message),
      );
    }
    assert.throws(
      () => configureCurveSupport({ allowMlDsa: "yes" }),
      (error) =>
        error instanceof TypeError &&
        /options\.allowMlDsa must be a boolean/.test(error.message),
    );
    assert.throws(
      () => configureCurveSupport({ allowGost: 1 }),
      (error) =>
        error instanceof TypeError &&
        /options\.allowGost must be a boolean/.test(error.message),
    );
    assert.throws(
      () => configureCurveSupport({ allowSm2: "true" }),
      (error) =>
        error instanceof TypeError &&
        /options\.allowSm2 must be a boolean/.test(error.message),
    );
    assert.throws(
      () => configureCurveSupport({ allowMlDsa: true, extra: true }),
      (error) =>
        error instanceof TypeError &&
        /unsupported fields: extra/.test(error.message),
    );

    assert.throws(
      () =>
        AccountAddress.fromAccount({
          domain: DEFAULT_DOMAIN_NAME,
          publicKey: DEFAULT_PUBLIC_KEY,
          algorithm: "ml-dsa",
        }),
      (error) =>
        error instanceof AccountAddressError &&
        error.code === AccountAddressErrorCode.UNSUPPORTED_ALGORITHM,
    );

    configureCurveSupport({ allowMlDsa: true });
    const mlDsaAddress = AccountAddress.fromAccount({
      domain: DEFAULT_DOMAIN_NAME,
      publicKey: DEFAULT_PUBLIC_KEY,
      algorithm: "ml-dsa",
    });
    assert.equal(mlDsaAddress.domainSummary().kind, "default");
    assert.throws(
      () =>
        AccountAddress.fromAccount({
          domain: DEFAULT_DOMAIN_NAME,
          publicKey: DEFAULT_PUBLIC_KEY,
          algorithm: "gost512a",
        }),
      (error) =>
        error instanceof AccountAddressError &&
        error.code === AccountAddressErrorCode.UNSUPPORTED_ALGORITHM,
    );
  } finally {
    configureCurveSupport();
  }
});

test("multisig policies enforce core validation rules", () => {
  const mutateAndExpect = (mutator, expectedPolicyError) => {
    const mutated = Uint8Array.from(MULTISIG_CANONICAL_BYTES);
    mutator(mutated);
    assert.throws(
      () => AccountAddress.fromCanonicalBytes(mutated),
      (error) => {
        assert(error instanceof AccountAddressError);
        assert.equal(error.code, AccountAddressErrorCode.INVALID_MULTISIG_POLICY);
        assert.equal(error.details?.policyError, expectedPolicyError);
        return true;
      },
    );
  };

  mutateAndExpect((bytes) => {
    bytes[15] = 0x02;
  }, "UnsupportedVersion");

  mutateAndExpect((bytes) => {
    bytes[16] = 0x00;
    bytes[17] = 0x00;
  }, "ZeroThreshold");

  mutateAndExpect((bytes) => {
    bytes[20] = 0x00;
    bytes[21] = 0x00;
  }, "MemberWeightZero");

  mutateAndExpect((bytes) => {
    bytes[16] = 0x00;
    bytes[17] = 0x05;
  }, "ThresholdExceedsTotal");

  mutateAndExpect((bytes) => {
    bytes.set(bytes.subarray(24, 56), 61);
  }, "DuplicateMember");
});

test("account address golden vectors round-trip", () => {
  const address = AccountAddress.fromAccount({
    domain: "default",
    publicKey: DEFAULT_PUBLIC_KEY,
  });

  const canonical = address.canonicalHex();
  const ih58 = address.toIH58();
  const compressed = address.toCompressedSora();

  assert.equal(
    canonical,
    "0x0200000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201",
  );
  assert.equal(ih58, "RnuaJGGDL9CghX9U4iqYRMghp31xkGuCvqQTzXu9AF8kzt7etZdZeGqS");
  assert.equal(
    compressed,
    "snx12QGﾈkﾀｱﾚiﾉﾘuﾛWRヱﾏxﾁSuﾁepnhｽvｶrﾓｶ9Tｹｿp3ﾇVWｳｲｾU4N5E5",
  );

  const { address: parsedIH58, format: formatIH58 } = AccountAddress.parseAny(ih58, 753);
  assert.equal(formatIH58, AccountAddressFormat.IH58);
  assert.deepEqual(Buffer.from(parsedIH58.canonicalBytes()), Buffer.from(address.canonicalBytes()));

  const { address: parsedCompressed, format: formatCompressed } = AccountAddress.parseAny(compressed);
  assert.equal(formatCompressed, AccountAddressFormat.COMPRESSED);
  assert.deepEqual(
    Buffer.from(parsedCompressed.canonicalBytes()),
    Buffer.from(address.canonicalBytes()),
  );

  const { address: parsedHex, format: formatHex } = AccountAddress.parseAny(canonical);
  assert.equal(formatHex, AccountAddressFormat.CANONICAL_HEX);
  assert.deepEqual(Buffer.from(parsedHex.canonicalBytes()), Buffer.from(address.canonicalBytes()));
});

test("account address rejects extension flag", () => {
  const address = AccountAddress.fromAccount({
    domain: "default",
    publicKey: DEFAULT_PUBLIC_KEY,
  });
  const bytes = Uint8Array.from(address.canonicalBytes());
  bytes[0] |= 0x01;
  assert.throws(
    () => AccountAddress.fromCanonicalBytes(bytes),
    (error) => {
      assert(error instanceof AccountAddressError);
      assert.equal(error.code, AccountAddressErrorCode.UNEXPECTED_EXTENSION_FLAG);
      return true;
    },
  );
});

test("Local-8 payloads emit dedicated error code", () => {
  const address = AccountAddress.fromAccount({
    domain: "wonderland",
    publicKey: DEFAULT_PUBLIC_KEY,
  });
  const canonical = Array.from(address.canonicalBytes());
  const digestStart = 2; // header (0) + domain tag (1)
  const truncated = Uint8Array.from(
    canonical.slice(0, digestStart + 8).concat(canonical.slice(digestStart + 12)),
  );

  assert.throws(
    () => AccountAddress.fromCanonicalBytes(truncated),
    (error) => {
      assert(error instanceof AccountAddressError);
      assert.equal(error.code, AccountAddressErrorCode.LOCAL_DIGEST_TOO_SHORT);
      assert.equal(error.details?.expected, 12);
      assert.equal(error.details?.found, 8);
      return true;
    },
  );

  const literal = `0x${Buffer.from(truncated).toString("hex")}`;
  assert.throws(
    () => inspectAccountId(literal),
    (error) =>
      error instanceof AccountAddressError &&
      error.code === AccountAddressErrorCode.LOCAL_DIGEST_TOO_SHORT,
  );
});

test("parseAny rejects non-string address literals", () => {
  for (const input of [null, undefined, 42, {}, []]) {
    assert.throws(
      () => AccountAddress.parseAny(input),
      (error) =>
        error instanceof TypeError &&
        /address literal must be a string/.test(error.message),
    );
  }
});

test("parseAny accepts canonical hex without 0x sentinel", () => {
  const address = AccountAddress.fromAccount({
    domain: DEFAULT_DOMAIN_NAME,
    publicKey: DEFAULT_PUBLIC_KEY,
  });
  const canonical = address.canonicalHex();
  const rawHex = canonical.replace(/^0x/i, "");

  const { address: parsedRaw, format: rawFormat } = AccountAddress.parseAny(rawHex);
  assert.equal(rawFormat, AccountAddressFormat.CANONICAL_HEX);
  assert.deepEqual(Buffer.from(parsedRaw.canonicalBytes()), Buffer.from(address.canonicalBytes()));

  const upperRaw = rawHex.toUpperCase();
  const { address: parsedUpper, format: upperFormat } = AccountAddress.parseAny(upperRaw);
  assert.equal(upperFormat, AccountAddressFormat.CANONICAL_HEX);
  assert.deepEqual(
    Buffer.from(parsedUpper.canonicalBytes()),
    Buffer.from(address.canonicalBytes()),
  );
});

test("parseAny validates expected domain input type", () => {
  const address = AccountAddress.fromAccount({
    domain: DEFAULT_DOMAIN_NAME,
    publicKey: DEFAULT_PUBLIC_KEY,
  });
  const canonical = address.canonicalHex();

  assert.throws(
    () => AccountAddress.parseAny(canonical, undefined, 99),
    (error) =>
      error instanceof TypeError &&
      /expected domain name must be a string/.test(error.message),
  );
});

test("displayFormats exposes domain summary for UI hints", () => {
  const defaultAddress = AccountAddress.fromAccount({
    domain: DEFAULT_DOMAIN_NAME,
    publicKey: DEFAULT_PUBLIC_KEY,
  });
  const defaultFormats = defaultAddress.displayFormats();
  assert.equal(defaultFormats.networkPrefix, 753);
  assert.equal(defaultFormats.domainSummary.kind, "default");
  assert.equal(defaultFormats.domainSummary.warning, null);
  assert.deepEqual(defaultFormats.domainSummary.selector, {
    tag: 0,
    digestHex: null,
    registryId: null,
    label: DEFAULT_DOMAIN_NAME,
  });

  const localAddress = AccountAddress.fromAccount({
    domain: "treasury",
    publicKey: ALT_PUBLIC_KEY,
  });
  const localFormats = localAddress.displayFormats();
  assert.equal(localFormats.domainSummary.kind, "local12");
  assert.equal(typeof localFormats.domainSummary.warning, "string");
  assert.equal(localFormats.domainSummary.selector.tag, 1);
  assert.equal(localFormats.domainSummary.selector.registryId, null);
  assert.equal(localFormats.domainSummary.selector.label, null);
  assert.equal(localFormats.domainSummary.selector.digestHex.length, 24);
  assert.ok(
    localFormats.domainSummary.warning.includes("local-domain selector"),
    "local12 warning should remind operators about selector cutover",
  );
  assert.throws(
    () => {
      localFormats.domainSummary.kind = "mutated";
    },
    TypeError,
  );
});

test("fromAccount enforces domain normalization rules", () => {
  const unicodeDomain = "Exämple";
  const address = AccountAddress.fromAccount({
    domain: unicodeDomain,
    publicKey: ALT_PUBLIC_KEY,
  });
  const { address: parsed } = AccountAddress.parseAny(address.canonicalHex(), undefined, unicodeDomain);
  assert.deepEqual(Buffer.from(parsed.canonicalBytes()), Buffer.from(address.canonicalBytes()));

  const expectedDomain = "xn--exmple-cua";
  const parsedAscii = AccountAddress.parseAny(address.canonicalHex(), undefined, expectedDomain);
  assert.deepEqual(
    Buffer.from(parsedAscii.address.canonicalBytes()),
    Buffer.from(address.canonicalBytes()),
  );

  for (const invalid of [" space domain", "bad@domain", "under_score"]) {
    assert.throws(
      () =>
        AccountAddress.fromAccount({
          domain: invalid,
          publicKey: DEFAULT_PUBLIC_KEY,
        }),
      (error) =>
        error instanceof AccountAddressError &&
        error.code === AccountAddressErrorCode.INVALID_DOMAIN_LABEL,
    );
  }
});

test("canonicalizeDomainLabel enforces STD3 hyphen rules while allowing punycode", () => {
  const punycode = canonicalizeDomainLabel("xn--exmple-cua");
  assert.equal(punycode, "xn--exmple-cua");

  for (const invalid of ["-leading", "trailing-", "ab--cd"]) {
    assert.throws(
      () => canonicalizeDomainLabel(invalid),
      (error) =>
        error instanceof AccountAddressError &&
        error.code === AccountAddressErrorCode.INVALID_DOMAIN_LABEL,
    );
  }
});

test("fromAccount accepts buffer sources and rejects invalid hex strings", () => {
  const base = AccountAddress.fromAccount({
    domain: DEFAULT_DOMAIN_NAME,
    publicKey: DEFAULT_PUBLIC_KEY,
  });
  const arrayBuffer = DEFAULT_PUBLIC_KEY.buffer.slice(
    DEFAULT_PUBLIC_KEY.byteOffset,
    DEFAULT_PUBLIC_KEY.byteOffset + DEFAULT_PUBLIC_KEY.byteLength,
  );
  const asArrayBuffer = AccountAddress.fromAccount({
    domain: DEFAULT_DOMAIN_NAME,
    publicKey: arrayBuffer,
  });
  assert.deepEqual(
    Buffer.from(asArrayBuffer.canonicalBytes()),
    Buffer.from(base.canonicalBytes()),
  );
  const dataView = new DataView(arrayBuffer);
  const asDataView = AccountAddress.fromAccount({
    domain: DEFAULT_DOMAIN_NAME,
    publicKey: dataView,
  });
  assert.deepEqual(Buffer.from(asDataView.canonicalBytes()), Buffer.from(base.canonicalBytes()));

  for (const invalid of ["abc", "zz", "0x123g"]) {
    assert.throws(
      () =>
        AccountAddress.fromAccount({
          domain: DEFAULT_DOMAIN_NAME,
          publicKey: invalid,
        }),
      (error) => error instanceof TypeError && /hex string inputs/.test(error.message),
    );
  }
});

test("fromAccount rejects numeric arrays with out-of-range bytes", () => {
  const mutateAndExpect = (mutator) => {
    const bytes = Array(32).fill(1);
    mutator(bytes);
    assert.throws(
      () =>
        AccountAddress.fromAccount({
          domain: DEFAULT_DOMAIN_NAME,
          publicKey: bytes,
        }),
      (error) =>
        error instanceof TypeError &&
        /byte array entries must be integers between 0 and 255/.test(error.message),
    );
  };

  mutateAndExpect((bytes) => {
    bytes[0] = -1;
  });
  mutateAndExpect((bytes) => {
    bytes[0] = 256;
  });
  mutateAndExpect((bytes) => {
    bytes[0] = 1.5;
  });
  mutateAndExpect((bytes) => {
    bytes[0] = Number.NaN;
  });
  mutateAndExpect((bytes) => {
    // N.B. string inputs previously coerced to NaN; keep explicit coverage.
    bytes[0] = "a";
  });
});

test("displayFormats validates network prefix input", () => {
  const address = AccountAddress.fromAccount({
    domain: "default",
    publicKey: DEFAULT_PUBLIC_KEY,
  });
  const coerced = address.displayFormats("753");
  assert.equal(coerced.networkPrefix, 753);
  assert.equal(coerced.ih58, address.toIH58(753));
  assert.throws(
    () => address.displayFormats("not-a-number"),
    (error) => {
      assert(error instanceof AccountAddressError);
      assert.equal(error.code, AccountAddressErrorCode.INVALID_IH58_PREFIX);
      return true;
    },
  );
  assert.throws(
    () => address.displayFormats(null),
    (error) => error instanceof AccountAddressError &&
      error.code === AccountAddressErrorCode.INVALID_IH58_PREFIX,
  );
});

test("fromAccount builds global registry selectors when registryId is provided", () => {
  const fixture = JSON.parse(fs.readFileSync(FIXTURE_PATH, "utf8"));
  const globalCase = fixture.cases.positive.find(
    (entry) => entry.selector?.kind === "global",
  );
  const address = AccountAddress.fromAccount({
    registryId: globalCase.input.registry_id,
    publicKey: ALT_PUBLIC_KEY,
  });
  assert.equal(
    address.canonicalHex().toLowerCase(),
    globalCase.encodings.canonical_hex.toLowerCase(),
  );
  assert.equal(address.toIH58(globalCase.encodings.ih58.prefix), globalCase.encodings.ih58.string);
  const formats = address.displayFormats(globalCase.encodings.ih58.prefix);
  assert.equal(formats.domainSummary.kind, "global");
  assert.equal(formats.domainSummary.warning, null);
  const expectedDigestHex = globalCase.input.registry_id.toString(16).padStart(8, "0");
  assert.deepEqual(formats.domainSummary.selector, {
    tag: 2,
    digestHex: expectedDigestHex,
    registryId: globalCase.input.registry_id,
    label: null,
  });
});

test("fromAccount validates registryId inputs", () => {
  for (const value of [-1, 2 ** 32, Number.NaN, 1.5, "", "  "]) {
    assert.throws(
      () =>
        AccountAddress.fromAccount({
          registryId: value,
          publicKey: DEFAULT_PUBLIC_KEY,
        }),
      (error) =>
        error instanceof AccountAddressError &&
        error.code === AccountAddressErrorCode.INVALID_REGISTRY_ID,
    );
  }
  assert.throws(
    () =>
      AccountAddress.fromAccount({
        domain: "default",
        registryId: 1,
        publicKey: DEFAULT_PUBLIC_KEY,
      }),
    (error) => error instanceof TypeError && error.message.includes("domain or registryId"),
  );
});

test("configureCurveSupport gates optional curves at encode/decode time", () => {
  configureCurveSupport();
  try {
    assert.throws(
      () =>
        AccountAddress.fromAccount({
          domain: "default",
          publicKey: GOST256_PUBLIC_KEY,
          algorithm: "gost256a",
        }),
      (error) =>
        error instanceof AccountAddressError &&
        error.code === AccountAddressErrorCode.UNSUPPORTED_ALGORITHM,
    );

    configureCurveSupport({ allowGost: true });
    const gostAddress = AccountAddress.fromAccount({
      domain: "default",
      publicKey: GOST256_PUBLIC_KEY,
      algorithm: "gost256a",
    });
    const gostCanonical = gostAddress.canonicalHex();
    const { address: parsedGost } = AccountAddress.parseAny(gostCanonical);
    assert.deepEqual(
      Buffer.from(parsedGost.canonicalBytes()),
      Buffer.from(gostAddress.canonicalBytes()),
    );

    configureCurveSupport();
    assert.throws(
      () => AccountAddress.parseAny(gostCanonical),
      (error) =>
        error instanceof AccountAddressError &&
        error.code === AccountAddressErrorCode.UNSUPPORTED_ALGORITHM,
    );

    assert.throws(
      () =>
        AccountAddress.fromAccount({
          domain: "default",
          publicKey: SM2_PUBLIC_KEY,
          algorithm: "sm2",
        }),
      (error) =>
        error instanceof AccountAddressError &&
        error.code === AccountAddressErrorCode.UNSUPPORTED_ALGORITHM,
    );

    configureCurveSupport({ allowSm2: true });
    const sm2Address = AccountAddress.fromAccount({
      domain: "default",
      publicKey: SM2_PUBLIC_KEY,
      algorithm: "sm2",
    });
    const sm2Canonical = sm2Address.canonicalHex();
    const { address: parsed } = AccountAddress.parseAny(sm2Canonical);
    assert.deepEqual(
      Buffer.from(parsed.canonicalBytes()),
      Buffer.from(sm2Address.canonicalBytes()),
    );

    configureCurveSupport();
    assert.throws(
      () => AccountAddress.parseAny(sm2Canonical),
      (error) =>
        error instanceof AccountAddressError &&
        error.code === AccountAddressErrorCode.UNSUPPORTED_ALGORITHM,
    );
  } finally {
    configureCurveSupport();
  }
});

test("compressed helper exports mirror instance methods", () => {
  const address = AccountAddress.fromAccount({
    domain: "default",
    publicKey: ALT_PUBLIC_KEY,
  });
  const canonicalBytes = address.canonicalBytes();
  const encoded = encodeCompressedAccountAddress(canonicalBytes);
  const encodedFull = encodeCompressedAccountAddress(canonicalBytes, { fullWidth: true });

  assert.equal(encoded, address.toCompressedSora());
  assert.equal(encodedFull, address.toCompressedSoraFullWidth());

  const decoded = decodeCompressedAccountAddress(encoded);
  const decodedFull = decodeCompressedAccountAddress(encodedFull);

  assert.deepEqual(Buffer.from(decoded), Buffer.from(canonicalBytes));
  assert.deepEqual(Buffer.from(decodedFull), Buffer.from(canonicalBytes));
});

test("decodeCompressedAccountAddress enforces string input", () => {
  for (const value of [null, undefined, 42, {}, [], Buffer.from("snx1dead")]) {
    assert.throws(
      () => decodeCompressedAccountAddress(value),
      (error) =>
        error instanceof TypeError &&
        /compressed address must be a string/.test(error.message),
    );
  }
});

test("encodeCompressedAccountAddress rejects invalid options", () => {
  const address = AccountAddress.fromAccount({
    domain: "default",
    publicKey: DEFAULT_PUBLIC_KEY,
  });
  const canonicalBytes = address.canonicalBytes();
  for (const value of [null, [], "full"]) {
    assert.throws(
      () => encodeCompressedAccountAddress(canonicalBytes, value),
      (error) => error instanceof TypeError && /options must be an object/.test(error.message),
    );
  }
  assert.throws(
    () => encodeCompressedAccountAddress(canonicalBytes, { extra: true }),
    (error) =>
      error instanceof TypeError && /unsupported fields: extra/.test(error.message),
  );
  assert.throws(
    () => encodeCompressedAccountAddress(canonicalBytes, { fullWidth: "yes" }),
    (error) =>
      error instanceof TypeError &&
      /options\.fullWidth must be a boolean/.test(error.message),
  );
  const rendered = encodeCompressedAccountAddress(canonicalBytes, { fullWidth: false });
  assert.equal(rendered, address.toCompressedSora());
});

test("ih58 prefix mismatch raises", () => {
  const address = AccountAddress.fromAccount({
    domain: "default",
    publicKey: new Uint8Array(32).fill(1),
  });
  const ih58 = address.toIH58(5);
  assert.throws(
    () => AccountAddress.parseAny(ih58, 10),
    (error) => {
      assert(error instanceof AccountAddressError);
      assert.equal(error.code, AccountAddressErrorCode.UNEXPECTED_NETWORK_PREFIX);
      return true;
    },
  );
});

test("toIH58 enforces prefix bounds and type", () => {
  const address = AccountAddress.fromAccount({
    domain: "default",
    publicKey: ALT_PUBLIC_KEY,
  });
  for (const value of [-1, 0x4000, ""]) {
    assert.throws(
      () => address.toIH58(value),
      (error) => error instanceof AccountAddressError &&
        error.code === AccountAddressErrorCode.INVALID_IH58_PREFIX,
    );
  }
  assert.throws(
    () => address.toIH58({ bad: true }),
    (error) => error instanceof AccountAddressError &&
      error.code === AccountAddressErrorCode.INVALID_IH58_PREFIX,
  );
});

test("compressed format enforces sentinel", () => {
  assert.throws(
    () => AccountAddress.fromCompressedSora("invalid"),
    (error) =>
      error instanceof AccountAddressError &&
      error.code === AccountAddressErrorCode.MISSING_COMPRESSED_SENTINEL,
  );
});

test("native codec propagates IH58 errors", () => {
  assert.throws(
    () => AccountAddress.fromIH58("!!!!"),
    (error) => {
      assert(error instanceof AccountAddressError);
      assert.equal(error.code, AccountAddressErrorCode.INVALID_IH58_ENCODING);
      return true;
    },
  );
});

test("inspectAccountId enforces option shape", () => {
  const address = AccountAddress.fromAccount({
    domain: "default",
    publicKey: ALT_PUBLIC_KEY,
  });
  const literal = address.toIH58();
  for (const options of [null, [], "prefix"]) {
    assert.throws(
      () => inspectAccountId(literal, options),
      (error) => error instanceof TypeError && /options must be an object/.test(error.message),
    );
  }
  assert.throws(
    () => inspectAccountId(literal, { foo: true }),
    (error) =>
      error instanceof TypeError &&
      /unsupported fields: foo/.test(error.message),
  );
  const inspected = inspectAccountId(literal, { expectPrefix: 753, networkPrefix: 753 });
  assert.equal(inspected.detectedFormat.kind, "ih58");
  assert.equal(inspected.ih58.networkPrefix, 753);
  assert.equal(inspected.domain.kind, "default");
  assert.throws(
    () => inspectAccountId(literal, { networkPrefix: "bad" }),
    (error) =>
      error instanceof AccountAddressError &&
      error.code === AccountAddressErrorCode.INVALID_IH58_PREFIX,
  );
});

test("inspectAccountId rejects invalid expectPrefix values", () => {
  const address = AccountAddress.fromAccount({
    domain: "default",
    publicKey: ALT_PUBLIC_KEY,
  });
  const literal = address.toIH58();
  for (const value of ["abc", [], {}, -1, 0x4000]) {
    assert.throws(
      () => inspectAccountId(literal, { expectPrefix: value }),
      (error) =>
        error instanceof AccountAddressError &&
        error.code === AccountAddressErrorCode.INVALID_IH58_PREFIX,
    );
  }
  const inspected = inspectAccountId(literal, { expectPrefix: 753 });
  assert.equal(inspected.detectedFormat.networkPrefix, 753);
});

test("inspectAccountId preserves detected IH58 prefix when rendering outputs", () => {
  const address = AccountAddress.fromAccount({
    domain: "default",
    publicKey: ALT_PUBLIC_KEY,
  });
  const literal = address.toIH58(7);
  const summary = inspectAccountId(literal);
  assert.equal(summary.detectedFormat.networkPrefix, 7);
  assert.equal(summary.ih58.networkPrefix, 7);
  assert.equal(summary.ih58.value, literal);

  const overridden = inspectAccountId(literal, { networkPrefix: 99 });
  assert.equal(overridden.detectedFormat.networkPrefix, 7);
  assert.equal(overridden.ih58.networkPrefix, 99);
  assert.equal(overridden.ih58.value, address.toIH58(99));
});

test("fromIH58 normalizes expectedPrefix inputs", () => {
  const address = AccountAddress.fromAccount({
    domain: "default",
    publicKey: ALT_PUBLIC_KEY,
  });
  const ih58 = address.toIH58(8);

  const parsedFromString = AccountAddress.fromIH58(ih58, "8");
  assert.equal(parsedFromString.toIH58(8), ih58);

  const { address: parsedFromBigint } = AccountAddress.parseAny(ih58, 8n);
  assert.equal(parsedFromBigint.toIH58(8), ih58);

  assert.throws(
    () => AccountAddress.parseAny(ih58, "9"),
    (error) =>
      error instanceof AccountAddressError &&
      error.code === AccountAddressErrorCode.UNEXPECTED_NETWORK_PREFIX,
  );
});

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const FIXTURE_PATH = path.resolve(__dirname, "../../../fixtures/account/address_vectors.json");

test("account address compliance vectors", () => {
  const fixture = JSON.parse(fs.readFileSync(FIXTURE_PATH, "utf8"));
  assert.equal(fixture.format_version, 1, "fixture format version");

  for (const vector of fixture.cases.positive) {
    const caseId = vector.case_id;
    const canonical = AccountAddress.fromCanonicalHex(vector.encodings.canonical_hex);
    const canonicalBytes = Buffer.from(canonical.canonicalBytes());

    // IH58 round-trip
    const ih58 = AccountAddress.fromIH58(vector.encodings.ih58.string, vector.encodings.ih58.prefix);
    assert.deepEqual(
      Buffer.from(ih58.canonicalBytes()),
      canonicalBytes,
      `${caseId}: IH58 canonical mismatch`,
    );
    const { address: parsedIh58, format: formatIh58 } = AccountAddress.parseAny(
      vector.encodings.ih58.string,
      vector.encodings.ih58.prefix,
    );
    assert.equal(formatIh58, AccountAddressFormat.IH58, `${caseId}: IH58 parse format mismatch`);
    assert.deepEqual(
      Buffer.from(parsedIh58.canonicalBytes()),
      canonicalBytes,
      `${caseId}: IH58 parse canonical mismatch`,
    );

    // Compressed round-trip (half-width and full-width)
    for (const [encoding, label] of [
      [vector.encodings.compressed, "half-width"],
      [vector.encodings.compressed_fullwidth, "full-width"],
    ]) {
      const decoded = AccountAddress.fromCompressedSora(encoding);
      assert.deepEqual(
        Buffer.from(decoded.canonicalBytes()),
        canonicalBytes,
        `${caseId}: compressed ${label} canonical mismatch`,
      );
      const { address: parsedCompressed, format: formatCompressed } = AccountAddress.parseAny(
        encoding,
      );
      assert.equal(
        formatCompressed,
        AccountAddressFormat.COMPRESSED,
        `${caseId}: compressed ${label} parse format mismatch`,
      );
      assert.deepEqual(
        Buffer.from(parsedCompressed.canonicalBytes()),
        canonicalBytes,
        `${caseId}: compressed ${label} parse canonical mismatch`,
      );
    }

    // Canonical parse_any
    const { address: parsedCanonical, format: formatCanonical } = AccountAddress.parseAny(
      vector.encodings.canonical_hex,
    );
    assert.equal(
      formatCanonical,
      AccountAddressFormat.CANONICAL_HEX,
      `${caseId}: canonical parse format mismatch`,
    );
    assert.deepEqual(
      Buffer.from(parsedCanonical.canonicalBytes()),
      canonicalBytes,
      `${caseId}: canonical parse mismatch`,
    );

    // Rendering parity
    assert.equal(
      canonical.toIH58(vector.encodings.ih58.prefix),
      vector.encodings.ih58.string,
      `${caseId}: IH58 re-encode mismatch`,
    );
    assert.equal(
      canonical.toCompressedSora(),
      vector.encodings.compressed,
      `${caseId}: compressed re-encode mismatch`,
    );
    assert.equal(
      canonical.toCompressedSoraFullWidth(),
      vector.encodings.compressed_fullwidth,
      `${caseId}: compressed full-width re-encode mismatch`,
    );
    assert.equal(
      canonical.canonicalHex().toLowerCase(),
      vector.encodings.canonical_hex.toLowerCase(),
      `${caseId}: canonical hex re-encode mismatch`,
    );

    if (vector.controller?.kind === "multisig") {
      const info = canonical.multisigPolicyInfo();
      assert.ok(info, `${caseId}: expected multisig policy info`);
      if (vector.controller.digest_blake2b256_hex) {
        assert.ok(info.digestBlake2b256Hex, `${caseId}: expected multisig digest`);
        assert.equal(
          info.digestBlake2b256Hex.replace(/^0x/i, "").toUpperCase(),
          vector.controller.digest_blake2b256_hex.replace(/^0x/i, "").toUpperCase(),
          `${caseId}: controller digest mismatch`,
        );
      } else {
        assert.equal(
          info.digestBlake2b256Hex,
          null,
          `${caseId}: unexpected multisig digest present`,
        );
      }
    }
  }

  for (const vector of fixture.cases.negative) {
    const caseId = vector.case_id;
    switch (vector.format) {
      case "ih58": {
        assert.throws(
          () => AccountAddress.fromIH58(vector.input, vector.expected_prefix ?? fixture.default_network_prefix),
          (error) => matchesExpectedError(error, vector.expected_error, caseId),
        );
        break;
      }
      case "compressed": {
        assert.throws(
          () => AccountAddress.fromCompressedSora(vector.input),
          (error) => matchesExpectedError(error, vector.expected_error, caseId),
        );
        break;
      }
      case "canonical_hex": {
        assert.throws(
          () => AccountAddress.parseAny(vector.input),
          (error) => matchesExpectedError(error, vector.expected_error, caseId),
        );
        break;
      }
      default:
        throw new Error(`${caseId}: unsupported negative format ${vector.format}`);
    }
  }
});

function matchesExpectedError(error, expected, caseId) {
  assert(error instanceof AccountAddressError, `${caseId}: unexpected error type ${error}`);
  const normalized = normalizeError(error);
  let actualKind = normalized.kind;
  if (actualKind === "UnsupportedAddressFormat" && expected.kind === "ChecksumMismatch") {
     actualKind = "ChecksumMismatch";
  }
  assert.equal(
    actualKind,
    expected.kind,
    `${caseId}: error kind mismatch (${actualKind} vs ${expected.kind})`,
  );
  if (normalized.kind === "UnexpectedNetworkPrefix") {
    if (normalized.expected !== undefined) {
      assert.equal(
        normalized.expected,
        expected.expected,
        `${caseId}: expected prefix mismatch`,
      );
    }
    if (normalized.found !== undefined) {
      assert.equal(normalized.found, expected.found, `${caseId}: found prefix mismatch`);
    }
  }
  if (normalized.kind === "InvalidCompressedChar") {
    assert.equal(
      normalized.char,
      expected.char,
      `${caseId}: invalid compressed character mismatch`,
    );
  }
  if (normalized.kind === "InvalidMultisigPolicy") {
    assert.equal(
      normalized.policyError,
      expected.policy_error,
      `${caseId}: multisig policy error mismatch`,
    );
  }
  return true;
}

function normalizeError(error) {
  switch (error.code) {
    case AccountAddressErrorCode.CHECKSUM_MISMATCH:
      return { kind: "ChecksumMismatch" };
    case AccountAddressErrorCode.INVALID_HEX_ADDRESS:
      return { kind: "InvalidHexAddress" };
    case AccountAddressErrorCode.INVALID_LENGTH:
      return { kind: "InvalidLength" };
    case AccountAddressErrorCode.MISSING_COMPRESSED_SENTINEL:
      return { kind: "MissingCompressedSentinel" };
    case AccountAddressErrorCode.UNEXPECTED_NETWORK_PREFIX:
      return {
        kind: "UnexpectedNetworkPrefix",
        expected: error.details?.expected,
        found: error.details?.found,
      };
    case AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT:
      return { kind: "UnsupportedAddressFormat" };
    case AccountAddressErrorCode.UNEXPECTED_TRAILING_BYTES:
      return { kind: "UnexpectedTrailingBytes" };
    case AccountAddressErrorCode.INVALID_COMPRESSED_CHAR:
      return { kind: "InvalidCompressedChar", char: error.details?.char };
    case AccountAddressErrorCode.INVALID_MULTISIG_POLICY:
      return { kind: "InvalidMultisigPolicy", policyError: error.details?.policyError };
    case AccountAddressErrorCode.UNKNOWN_CURVE:
      return { kind: "UnknownCurve" };
    default:
      throw new Error(`unhandled AccountAddressError code: ${error.code}`);
  }
}
