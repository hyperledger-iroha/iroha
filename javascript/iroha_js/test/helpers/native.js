import { getNativeBinding } from "../../src/native.js";

const binding = getNativeBinding();

export const nativeBinding = binding;
export const hasNativeBinding = binding !== null;
export const nativeSkipMessage =
  "native iroha_js_host binding unavailable; run `npm run build:native`";

function normalizeArgs(nameOrOptions, optionsOrFn, maybeFn) {
  if (typeof nameOrOptions === "function") {
    return { name: nameOrOptions, options: { skip: nativeSkipMessage }, fn: undefined };
  }
  if (typeof optionsOrFn === "function" || optionsOrFn === undefined) {
    return {
      name: nameOrOptions,
      options: { skip: nativeSkipMessage },
      fn: optionsOrFn,
    };
  }
  return {
    name: nameOrOptions,
    options: { ...optionsOrFn, skip: optionsOrFn.skip ?? nativeSkipMessage },
    fn: maybeFn,
  };
}

export function makeNativeTest(baseTest) {
  if (hasNativeBinding) {
    return baseTest;
  }
  const wrapper = (nameOrOptions, optionsOrFn, maybeFn) => {
    const { name, options, fn } = normalizeArgs(nameOrOptions, optionsOrFn, maybeFn);
    return baseTest(name, options, fn);
  };
  wrapper.only = baseTest.only?.bind(baseTest);
  wrapper.skip = baseTest.skip?.bind(baseTest);
  wrapper.todo = baseTest.todo?.bind(baseTest);
  return wrapper;
}
