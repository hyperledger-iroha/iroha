import { getNativeBinding } from "../../src/native.js";

let binding = null;
let bindingError = null;
try {
  binding = getNativeBinding();
} catch (error) {
  bindingError = error;
}

const NORITO_REQUIRED_METHODS = Object.freeze([
  "noritoEncodeInstruction",
  "noritoDecodeInstruction",
]);
const SM2_REQUIRED_METHODS = Object.freeze([
  "sm2Keypair",
  "sm2KeypairFromSeed",
  "sm2KeypairFromPrivate",
  "sm2Sign",
  "sm2Verify",
  "sm2PublicKeyMultihash",
  "sm2FixtureFromSeed",
]);

export const nativeBinding = binding;
export const nativeBindingError = bindingError;
export const hasNativeBinding = binding !== null;
export const noritoRequiredMethods = NORITO_REQUIRED_METHODS;
export const sm2RequiredMethods = SM2_REQUIRED_METHODS;
export const nativeUnavailableMessage =
  "native iroha_js_host binding unavailable; run `npm run build:native`";

function hasNativeMethods(binding, required) {
  if (!binding) {
    return false;
  }
  if (!required) {
    return true;
  }
  if (typeof required === "function") {
    return Boolean(required(binding));
  }
  const methods = Array.isArray(required) ? required : [required];
  return methods.every((method) => typeof binding[method] === "function");
}

export function hasNoritoBinding(bindingOverride = nativeBinding) {
  return hasNativeMethods(bindingOverride, NORITO_REQUIRED_METHODS);
}

export function hasSm2Binding(bindingOverride = nativeBinding) {
  return hasNativeMethods(bindingOverride, SM2_REQUIRED_METHODS);
}

function missingNativeMethods(binding, required) {
  if (!binding || !required || typeof required === "function") {
    return [];
  }
  const methods = Array.isArray(required) ? required : [required];
  return methods.filter((method) => typeof binding[method] !== "function");
}

function nativeRequirementError(binding, required, bindingLoadError) {
  let message;
  if (!binding) {
    message = bindingLoadError
      ? `${nativeUnavailableMessage}: ${bindingLoadError.message}`
      : nativeUnavailableMessage;
  } else if (typeof required === "function") {
    message =
      "native iroha_js_host binding does not satisfy the required capability predicate";
  } else {
    const missing = missingNativeMethods(binding, required);
    message =
      `native iroha_js_host binding is missing required method(s): ${missing.join(", ")}`;
  }
  const error = new Error(message);
  error.code = "ERR_IROHA_NATIVE_TEST_REQUIREMENT";
  if (bindingLoadError) {
    error.cause = bindingLoadError;
  }
  return error;
}

function registerNativeRequirementFailure(
  baseTest,
  createError,
  nameOrOptions,
  optionsOrFn,
  maybeFn,
) {
  const fail = () => {
    throw createError();
  };
  if (typeof nameOrOptions === "function") {
    return baseTest(nameOrOptions.name || "native binding requirement", fail);
  }
  if (typeof optionsOrFn === "function" || optionsOrFn === undefined) {
    return baseTest(nameOrOptions, fail);
  }
  return baseTest(nameOrOptions, optionsOrFn, fail);
}

export function makeNativeTest(baseTest, options = {}) {
  const { require: required } = options;
  const bindingWasOverridden = Object.prototype.hasOwnProperty.call(
    options,
    "binding",
  );
  const effectiveBinding = bindingWasOverridden
    ? options.binding
    : nativeBinding;
  const canRun = hasNativeMethods(effectiveBinding, required);
  if (canRun) {
    return baseTest;
  }
  const bindingLoadError = bindingWasOverridden ? null : nativeBindingError;
  const createError = () =>
    nativeRequirementError(effectiveBinding, required, bindingLoadError);
  const wrapper = (nameOrOptions, optionsOrFn, maybeFn) => {
    return registerNativeRequirementFailure(
      baseTest,
      createError,
      nameOrOptions,
      optionsOrFn,
      maybeFn,
    );
  };
  wrapper.only =
    typeof baseTest.only === "function"
      ? (nameOrOptions, optionsOrFn, maybeFn) =>
          registerNativeRequirementFailure(
            baseTest.only.bind(baseTest),
            createError,
            nameOrOptions,
            optionsOrFn,
            maybeFn,
          )
      : undefined;
  wrapper.skip = baseTest.skip?.bind(baseTest);
  wrapper.todo = baseTest.todo?.bind(baseTest);
  return wrapper;
}
