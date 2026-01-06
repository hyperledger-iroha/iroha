"use strict";

import { compareUtf16 } from "./ordering.js";

function normalizePositiveInteger(value, context, { allowZero = false, max = Number.MAX_SAFE_INTEGER } = {}) {
  if (value === null || value === undefined) {
    throw new TypeError(`${context} is required`);
  }

  const numeric =
    typeof value === "bigint"
      ? Number(value)
      : typeof value === "number"
        ? value
        : (() => {
            throw new TypeError(`${context} must be a number or bigint`);
          })();

  if (!Number.isFinite(numeric) || !Number.isInteger(numeric)) {
    throw new TypeError(`${context} must be a finite integer`);
  }
  if (!allowZero && numeric <= 0) {
    throw new RangeError(`${context} must be greater than zero`);
  }
  if (allowZero && numeric < 0) {
    throw new RangeError(`${context} must be non-negative`);
  }
  if (numeric > max) {
    throw new RangeError(`${context} exceeds the supported maximum (${max})`);
  }
  return numeric;
}

function safeExpiry(nowMs, ttlMs) {
  const sum = nowMs + ttlMs;
  if (!Number.isFinite(sum) || sum > Number.MAX_SAFE_INTEGER) {
    return Number.MAX_SAFE_INTEGER;
  }
  return sum;
}

function assertNonEmptyString(value, context) {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new TypeError(`${context} must be a non-empty string`);
  }
  return value.trim();
}

class MultisigSpec {
  constructor(signatories, quorum, transactionTtlMs) {
    this.signatories = signatories;
    this.quorum = quorum;
    this.transactionTtlMs = transactionTtlMs;
  }

  previewProposalExpiry({ requestedTtlMs = null, nowMs = Date.now() } = {}) {
    const normalizedNow = normalizePositiveInteger(nowMs, "nowMs", { allowZero: true });
    const cap = this.transactionTtlMs;
    const requested =
      requestedTtlMs === null || requestedTtlMs === undefined
        ? null
        : normalizePositiveInteger(requestedTtlMs, "requestedTtlMs");

    const wasCapped = requested !== null && requested > cap;
    const effectiveTtlMs = requested !== null ? Math.min(requested, cap) : cap;
    const expiresAtMs = safeExpiry(normalizedNow, effectiveTtlMs);

    return {
      effectiveTtlMs,
      policyCapMs: cap,
      expiresAtMs,
      wasCapped,
    };
  }

  enforceProposalTtl({ requestedTtlMs = null, nowMs = Date.now() } = {}) {
    const preview = this.previewProposalExpiry({ requestedTtlMs, nowMs });
    if (preview.wasCapped && requestedTtlMs !== null && requestedTtlMs !== undefined) {
      throw new RangeError(
        `Requested multisig TTL ${requestedTtlMs} ms exceeds the policy cap ${preview.policyCapMs} ms; choose a value at or below the cap.`,
      );
    }
    return preview;
  }

  toPayload() {
    const sortedEntries = Array.from(this.signatories.entries()).sort(([left], [right]) =>
      compareUtf16(left, right),
    );
    const sortedSignatories = Object.fromEntries(sortedEntries);
    return {
      signatories: sortedSignatories,
      quorum: this.quorum,
      transaction_ttl_ms: this.transactionTtlMs,
    };
  }

  toJSON(prettyPrinted = false) {
    const payload = this.toPayload();
    return JSON.stringify(payload, null, prettyPrinted ? 2 : undefined);
  }
}

export class MultisigSpecBuilder {
  constructor() {
    this._signatories = new Map();
    this._quorum = null;
    this._transactionTtlMs = null;
  }

  setQuorum(quorum) {
    const normalized = normalizePositiveInteger(quorum, "quorum");
    if (normalized > 0xffff) {
      throw new RangeError("quorum must fit in a 16-bit unsigned integer");
    }
    this._quorum = normalized;
    return this;
  }

  setTransactionTtlMs(ttlMs) {
    const normalized = normalizePositiveInteger(ttlMs, "transactionTtlMs");
    this._transactionTtlMs = normalized;
    return this;
  }

  addSignatory(accountId, weight) {
    const normalizedAccount = assertNonEmptyString(accountId, "accountId");
    const normalizedWeight = normalizePositiveInteger(weight, "weight", { max: 0xff });
    this._signatories.set(normalizedAccount, normalizedWeight);
    return this;
  }

  removeSignatory(accountId) {
    const normalizedAccount = assertNonEmptyString(accountId, "accountId");
    this._signatories.delete(normalizedAccount);
    return this;
  }

  build() {
    if (this._quorum === null) {
      throw new Error("quorum must be set before building a multisig spec");
    }
    if (this._transactionTtlMs === null) {
      throw new Error("transactionTtlMs must be set before building a multisig spec");
    }
    if (this._signatories.size === 0) {
      throw new Error("multisig specs require at least one signatory");
    }
    if (this._signatories.size > 0xff) {
      throw new Error("multisig specs support at most 255 signatories");
    }

    let totalWeight = 0;
    for (const weight of this._signatories.values()) {
      totalWeight += weight;
    }
    if (totalWeight < this._quorum) {
      throw new RangeError(
        `quorum ${this._quorum} exceeds total signatory weight ${totalWeight}`,
      );
    }

    return new MultisigSpec(
      new Map(this._signatories.entries()),
      this._quorum,
      this._transactionTtlMs,
    );
  }

  toJSON(prettyPrinted = false) {
    return this.build().toJSON(prettyPrinted);
  }
}

export { MultisigSpec };
