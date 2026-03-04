import { getNativeBinding } from "../../src/native.js";

const binding = getNativeBinding();

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
export const hasNativeBinding = binding !== null;
export const noritoRequiredMethods = NORITO_REQUIRED_METHODS;
export const sm2RequiredMethods = SM2_REQUIRED_METHODS;
export const nativeSkipMessage =
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

function normalizeArgs(skipMessage, nameOrOptions, optionsOrFn, maybeFn) {
  if (typeof nameOrOptions === "function") {
    return { name: nameOrOptions, options: { skip: skipMessage }, fn: undefined };
  }
  if (typeof optionsOrFn === "function" || optionsOrFn === undefined) {
    return {
      name: nameOrOptions,
      options: { skip: skipMessage },
      fn: optionsOrFn,
    };
  }
  return {
    name: nameOrOptions,
    options: { ...optionsOrFn, skip: optionsOrFn.skip ?? skipMessage },
    fn: maybeFn,
  };
}

export function makeNativeTest(baseTest, options = {}) {
  const {
    require: required,
    binding: bindingOverride,
    skipMessage,
  } = options;
  const effectiveBinding = bindingOverride ?? nativeBinding;
  const canRun = hasNativeMethods(effectiveBinding, required);
  if (canRun) {
    return baseTest;
  }
  const message = skipMessage ?? nativeSkipMessage;
  const wrapper = (nameOrOptions, optionsOrFn, maybeFn) => {
    const { name, options: testOptions, fn } = normalizeArgs(
      message,
      nameOrOptions,
      optionsOrFn,
      maybeFn,
    );
    return baseTest(name, testOptions, fn);
  };
  wrapper.only = baseTest.only?.bind(baseTest);
  wrapper.skip = baseTest.skip?.bind(baseTest);
  wrapper.todo = baseTest.todo?.bind(baseTest);
  return wrapper;
}
