import test from "node:test";
import assert from "node:assert/strict";
import { normalizeIvmFault } from "../src/ivmFault.js";
import { ToriiError } from "../src/toriiErrors.js";

const fault = () => ({
  kind: { kind: "PointerAbi", value: { kind: "WrongType", value: null } },
  site: {
    code_hash: "12".repeat(32),
    selector: { kind: "Generic", value: null },
    position: { kind: "Initialization", value: null },
  },
});

test("HTTP errors preserve the canonical typed fault without diagnostic strings", () => {
  const value = fault();
  const error = new ToriiError("execution failed", { details: { ivm_fault: value } });
  assert.deepEqual(error.details.ivm_fault, value);
  value.kind.value.kind = "InvalidAddress";
  assert.equal(error.details.ivm_fault.kind.value.kind, "WrongType");
  assert.throws(() => new ToriiError("bad wire", { details: { ivm_fault: { ...fault(), trace: "unbounded" } } }), /exactly/);
});

// `IvmFaultKindV1` in codec-index order (0..=27).
const FAULT_KINDS = [
  "OutOfGas", "MemoryLimitExceeded", "MemoryAccessViolation", "MisalignedAccess", "MemoryOutOfBounds",
  "DecodeError", "InvalidOpcode", "UnknownSyscall", "UnsupportedSyscall", "GasCostOverflow", "Numeric",
  "PointerAbi", "AssertionFailed", "ExceededMaxCycles", "InvalidMetadata", "MissingHalt",
  "NullifierAlreadyUsed", "PermissionDenied", "PrivacyViolation", "RegisterOutOfBounds", "NoritoInvalid",
  "AbiTypeNotAllowed", "HostOutputItemsExceeded", "HostOutputBytesExceeded", "AmxBudgetExceeded",
  "ReentrantCall", "CallDepthExceeded", "ControlFlowViolation",
];
// `NumericFaultV1` in ABI tag order (1..=14).
const NUMERIC_FAULTS = [
  "MantissaOverflow", "ScaleOverflow", "DivisionByZero", "RepeatingDecimal", "ExactDivisionScaleOverflow",
  "InvalidScale", "InexactConversion", "NegativeQuantity", "QuantityUnderflow", "InvalidRoundingMode",
  "InvalidFailureMode", "ReservedRegisterNonZero", "NegativeSquareRoot", "RegisterIntegerOverflow",
];
// `PointerAbiFaultV1` in ABI tag order (1..=11).
const POINTER_ABI_FAULTS = [
  "InvalidAddress", "UnknownType", "TypeNotAllowed", "WrongType", "InvalidEnvelopeVersion", "OversizedLength",
  "TruncatedEnvelope", "PayloadHashMismatch", "MalformedFrame", "SchemaMismatch", "NonCanonical",
];

test("every current fault category and exact subtype code is accepted", () => {
  assert.equal(FAULT_KINDS.length, 28);
  for (const category of FAULT_KINDS.filter((name) => name !== "Numeric" && name !== "PointerAbi")) {
    const value = fault();
    value.kind = { kind: category, value: null };
    assert.deepEqual(normalizeIvmFault(value, "fault").kind, { kind: category, value: null }, category);
  }
  for (const [category, codes] of [["Numeric", NUMERIC_FAULTS], ["PointerAbi", POINTER_ABI_FAULTS]]) {
    for (const code of codes) {
      const value = fault();
      value.kind = { kind: category, value: { kind: code, value: null } };
      assert.deepEqual(normalizeIvmFault(value, "fault").kind, value.kind, `${category}(${code})`);
    }
  }
});

test("retired categories and cross-category subtype codes are rejected", () => {
  for (const retired of ["InvalidVectorLength", "VectorExtensionDisabled", "ZkExtensionDisabled"]) {
    const value = fault();
    value.kind = { kind: retired, value: null };
    assert.throws(() => normalizeIvmFault(value, "fault"), /category/, retired);
  }
  for (const [category, code] of [["Numeric", "WrongType"], ["PointerAbi", "RegisterIntegerOverflow"], ["Numeric", "ControlFlowViolation"]]) {
    const value = fault();
    value.kind = { kind: category, value: { kind: code, value: null } };
    assert.throws(() => normalizeIvmFault(value, "fault"), /fault code/, `${category}(${code})`);
  }
  const unitWithPayload = fault();
  unitWithPayload.kind = { kind: "ControlFlowViolation", value: { kind: "DivisionByZero", value: null } };
  assert.throws(() => normalizeIvmFault(unitWithPayload, "fault"), /category/);
});

test("fault fields reject getters and unsupported tags before interpretation", () => {
  const value = fault();
  let invoked = false;
  Object.defineProperty(value.site, "code_hash", { enumerable: true, get() { invoked = true; return "12".repeat(32); } });
  assert.throws(() => normalizeIvmFault(value, "fault"), /data property/);
  assert.equal(invoked, false);
  for (const tag of ["", "divisionbyzero", "0", "OutOfGas "]) {
    const value = fault();
    value.kind.kind = tag;
    assert.throws(() => normalizeIvmFault(value, "fault"), /category/);
  }
});
