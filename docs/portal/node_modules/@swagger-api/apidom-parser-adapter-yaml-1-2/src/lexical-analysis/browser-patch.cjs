"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
var _ramda = require("ramda");
var _ramdaAdjunct = require("ramda-adjunct");
var _treeSitter = _interopRequireDefault(require("web-tree-sitter/tree-sitter.wasm"));
// @ts-ignore

// patch fetch() to let emscripten load the WASM file
const realFetch = globalThis.fetch;
if ((0, _ramdaAdjunct.isFunction)(realFetch)) {
  globalThis.fetch = (...args) => {
    // @ts-ignore
    if ((0, _ramdaAdjunct.isString)(args[0]) && args[0].endsWith('tree-sitter.wasm')) {
      // @ts-ignore
      return realFetch.apply(globalThis, [_treeSitter.default, (0, _ramda.tail)(args)]);
    }
    return realFetch.apply(globalThis, args);
  };
}