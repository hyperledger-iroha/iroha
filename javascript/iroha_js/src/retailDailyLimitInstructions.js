import { ensureCanonicalAccountId, normalizeAssetDefinitionId } from "./normalizers.js";
import { NumericV1 } from "./numericV1.js";
import { stringifyStrictLosslessIntegerJson } from "./strictLosslessJson.js";

const U64_MAX = 0xffff_ffff_ffff_ffffn;
const IDENTITY_DOMAIN_V1 = "iroha.bpng.retail-identity.v1";
const MONETARY_PURPOSES_V1 = new Set([
  "mint_to_reserve",
  "credit_retail",
  "defund_retail",
  "burn_reserve",
]);

function exactObject(value, fields, context) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new TypeError(`${context} must be a plain object`);
  }
  const present = Reflect.ownKeys(value);
  if (present.length !== fields.length || present.some((field) => !fields.includes(field))) {
    throw new TypeError(`${context} must contain exactly ${fields.join(", ")}`);
  }
  for (const field of fields) {
    const descriptor = Object.getOwnPropertyDescriptor(value, field);
    if (!descriptor?.enumerable || !("value" in descriptor)) {
      throw new TypeError(`${context}.${field} must be an enumerable data field`);
    }
  }
  return value;
}

function exactText(value, context) {
  if (typeof value !== "string" || value.length === 0 || value.trim() !== value) {
    throw new TypeError(`${context} must be exact nonempty text`);
  }
  return value;
}

function optionalText(value, context) {
  return value === null ? null : exactText(value, context);
}

function assetId(value, context) {
  const literal = exactText(value, context);
  const normalized = normalizeAssetDefinitionId(literal, context);
  if (literal !== normalized) throw new TypeError(`${context} must be canonical`);
  return literal;
}

function accountId(value, context) {
  const literal = exactText(value, context);
  const normalized = ensureCanonicalAccountId(literal, context);
  if (literal !== normalized) throw new TypeError(`${context} must be canonical I105`);
  return literal;
}

function positiveU64(value, context) {
  let parsed;
  if (typeof value === "bigint") {
    parsed = value;
  } else if (typeof value === "number" && Number.isSafeInteger(value)) {
    parsed = BigInt(value);
  } else if (typeof value === "string" && /^[1-9][0-9]*$/u.test(value)) {
    parsed = BigInt(value);
  } else {
    throw new TypeError(`${context} must be a positive exact u64`);
  }
  if (parsed <= 0n || parsed > U64_MAX) {
    throw new RangeError(`${context} must be a positive exact u64`);
  }
  return parsed;
}

function firstReleaseRevision(value, context) {
  const revision = positiveU64(value, context);
  if (revision !== 1n) {
    throw new TypeError(`${context} must be exactly 1 in the first release`);
  }
  return revision;
}

function nonzeroDigest(value, context) {
  if (!Array.isArray(value) || value.length !== 32) {
    throw new TypeError(`${context} must be an exact 32-byte array`);
  }
  const bytes = value.map((byte, index) => {
    if (!Number.isInteger(byte) || byte < 0 || byte > 255) {
      throw new TypeError(`${context}[${index}] must be a byte`);
    }
    return byte;
  });
  if (bytes.every((byte) => byte === 0)) {
    throw new TypeError(`${context} must be nonzero`);
  }
  return bytes;
}

function positiveQuantity(value, context, wholeOnly = false, maxScale = 2) {
  if (typeof value !== "string") {
    throw new TypeError(`${context} must be a canonical quantity string`);
  }
  let parsed;
  try {
    parsed = NumericV1.decodeQuantityJson(value);
  } catch {
    throw new TypeError(`${context} must be a canonical quantity string`);
  }
  if (parsed.mantissa <= 0n || parsed.toString() !== value) {
    throw new TypeError(`${context} must be a positive canonical quantity string`);
  }
  if (parsed.scale > maxScale) {
    throw new TypeError(`${context} exceeds PGK's scale ${maxScale}`);
  }
  if (wholeOnly && !/^[1-9][0-9]*$/u.test(value)) {
    throw new TypeError(`${context} must be positive whole Kina`);
  }
  return value;
}

function instructionJson(name, payload) {
  // DataSpaceId is an unsigned 64-bit integer. BPNG's assigned value is above
  // Number.MAX_SAFE_INTEGER, so builders return exact JSON text for the native
  // instruction adapter instead of a lossy JavaScript object.
  return stringifyStrictLosslessIntegerJson({ [name]: payload }, name);
}

/** Build exact first-release activation JSON for a fresh PGK definition. */
export function buildActivateRetailDailyLimitV1InstructionJson(options) {
  const { definition, policy } = exactObject(options, ["definition", "policy"], "retail activation");
  exactObject(definition, [
    "id", "name", "description", "alias", "spec", "mintable", "logo",
    "metadata", "balance_scope_policy", "owning_domain",
  ], "retail activation.definition");
  const definitionId = assetId(definition.id, "retail activation.definition.id");
  exactText(definition.name, "retail activation.definition.name");
  optionalText(definition.description, "retail activation.definition.description");
  optionalText(definition.alias, "retail activation.definition.alias");
  optionalText(definition.logo, "retail activation.definition.logo");
  exactObject(definition.spec, ["scale"], "retail activation.definition.spec");
  if (definition.spec.scale !== 2 || definition.mintable !== "Infinitely"
      || definition.balance_scope_policy !== "DataspaceRestricted") {
    throw new TypeError("retail activation requires scale 2, elastic mintability and dataspace-restricted balances");
  }
  exactObject(definition.metadata, [], "retail activation.definition.metadata");
  exactText(definition.owning_domain, "retail activation.definition.owning_domain");

  exactObject(policy, [
    "asset_definition_id", "physical_dataspace", "revision", "daily_cap",
    "identity_issuer", "identity_issuer_public_key", "monetary_issuer_account",
    "reserve_account", "institutional_exceptions",
  ], "retail activation.policy");
  if (assetId(policy.asset_definition_id, "retail activation.policy.asset_definition_id") !== definitionId) {
    throw new TypeError("retail activation definition and policy asset IDs disagree");
  }
  const physicalDataspace = positiveU64(policy.physical_dataspace, "retail activation.policy.physical_dataspace");
  const revision = firstReleaseRevision(policy.revision, "retail activation.policy.revision");
  const dailyCap = positiveQuantity(policy.daily_cap, "retail activation.policy.daily_cap", true);
  const identityIssuer = accountId(policy.identity_issuer, "retail activation.policy.identity_issuer");
  const identityIssuerPublicKey = exactText(
    policy.identity_issuer_public_key,
    "retail activation.policy.identity_issuer_public_key",
  );
  const monetaryIssuerAccount = accountId(
    policy.monetary_issuer_account,
    "retail activation.policy.monetary_issuer_account",
  );
  const reserveAccount = accountId(policy.reserve_account, "retail activation.policy.reserve_account");
  if (monetaryIssuerAccount === reserveAccount) {
    throw new TypeError("retail monetary issuer and reserve must be distinct accounts");
  }
  if (!Array.isArray(policy.institutional_exceptions) || policy.institutional_exceptions.length !== 0) {
    throw new TypeError("first-release retail activation admits no institutional exceptions");
  }
  return instructionJson("ActivateRetailDailyLimitV1", {
    definition: {
      id: definitionId,
      name: definition.name,
      description: definition.description,
      alias: definition.alias,
      spec: { scale: 2 },
      mintable: "Infinitely",
      logo: definition.logo,
      metadata: {},
      balance_scope_policy: "DataspaceRestricted",
      owning_domain: definition.owning_domain,
    },
    policy: {
      asset_definition_id: definitionId,
      physical_dataspace: physicalDataspace,
      revision,
      daily_cap: dailyCap,
      identity_issuer: identityIssuer,
      identity_issuer_public_key: identityIssuerPublicKey,
      monetary_issuer_account: monetaryIssuerAccount,
      reserve_account: reserveAccount,
      institutional_exceptions: [],
    },
  });
}

/** Wrap one already signed issuer attestation without replacing its signer. */
export function buildBindRetailIdentityV1InstructionJson(options) {
  const { attestation } = exactObject(options, ["attestation"], "retail identity binding");
  exactObject(attestation, ["body", "signature"], "retail identity binding.attestation");
  const { body } = attestation;
  exactObject(body, [
    "domain", "asset_definition_id", "physical_dataspace", "policy_revision",
    "account_id", "identity", "uniqueness_evidence_digest",
  ], "retail identity binding.attestation.body");
  if (body.domain !== IDENTITY_DOMAIN_V1) {
    throw new TypeError(`retail identity domain must be ${IDENTITY_DOMAIN_V1}`);
  }
  exactObject(body.identity, ["digest"], "retail identity binding.attestation.body.identity");
  const signature = exactText(attestation.signature, "retail identity binding.attestation.signature");
  if (!/^(?:[0-9A-F]{2})+$/u.test(signature)) {
    throw new TypeError("retail identity signature must be canonical uppercase hex");
  }
  return instructionJson("BindRetailIdentityV1", {
    attestation: {
      body: {
        domain: IDENTITY_DOMAIN_V1,
        asset_definition_id: assetId(body.asset_definition_id, "retail identity binding.asset_definition_id"),
        physical_dataspace: positiveU64(body.physical_dataspace, "retail identity binding.physical_dataspace"),
        policy_revision: firstReleaseRevision(body.policy_revision, "retail identity binding.policy_revision"),
        account_id: accountId(body.account_id, "retail identity binding.account_id"),
        identity: { digest: nonzeroDigest(body.identity.digest, "retail identity binding.identity.digest") },
        uniqueness_evidence_digest: nonzeroDigest(
          body.uniqueness_evidence_digest,
          "retail identity binding.uniqueness_evidence_digest",
        ),
      },
      signature,
    },
  });
}

/** Build one exact monetary movement; Core still verifies bank receipts. */
export function buildRetailMonetaryMovementV1InstructionJson(options) {
  const source = exactObject(options, [
    "assetDefinitionId", "purpose", "retailAccount", "amount", "operationDigest",
  ], "retail monetary movement");
  if (!MONETARY_PURPOSES_V1.has(source.purpose)) {
    throw new TypeError("retail monetary movement purpose is not a first-release purpose");
  }
  const needsRetailAccount = source.purpose === "credit_retail" || source.purpose === "defund_retail";
  if ((source.retailAccount === null) === needsRetailAccount) {
    throw new TypeError("retail account must be present only for credit or defund");
  }
  return instructionJson("RetailMonetaryMovementV1", {
    asset_definition_id: assetId(source.assetDefinitionId, "retail monetary movement.assetDefinitionId"),
    purpose: { purpose: source.purpose, value: null },
    retail_account: needsRetailAccount
      ? accountId(source.retailAccount, "retail monetary movement.retailAccount")
      : null,
    amount: positiveQuantity(source.amount, "retail monetary movement.amount"),
    operation_digest: nonzeroDigest(source.operationDigest, "retail monetary movement.operationDigest"),
  });
}
