import { tail } from 'ramda';
import { isString, isFunction } from 'ramda-adjunct';
// @ts-ignore
import treeSitterWasm from 'web-tree-sitter/tree-sitter.wasm';

// patch fetch() to let emscripten load the WASM file
const realFetch = globalThis.fetch;
if (isFunction(realFetch)) {
  globalThis.fetch = (...args) => {
    // @ts-ignore
    if (isString(args[0]) && args[0].endsWith('tree-sitter.wasm')) {
      // @ts-ignore
      return realFetch.apply(globalThis, [treeSitterWasm, tail(args)]);
    }
    return realFetch.apply(globalThis, args);
  };
}