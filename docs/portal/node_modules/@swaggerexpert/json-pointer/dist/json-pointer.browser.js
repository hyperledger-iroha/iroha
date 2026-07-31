(function webpackUniversalModuleDefinition(root, factory) {
	if(typeof exports === 'object' && typeof module === 'object')
		module.exports = factory();
	else if(typeof define === 'function' && define.amd)
		define([], factory);
	else if(typeof exports === 'object')
		exports["JSONPointer"] = factory();
	else
		root["JSONPointer"] = factory();
})(self, () => {
return /******/ (() => { // webpackBootstrap
/******/ 	"use strict";
/******/ 	var __webpack_modules__ = ({

/***/ 58:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
/* harmony import */ var _escape_js__WEBPACK_IMPORTED_MODULE_1__ = __webpack_require__(980);
/* harmony import */ var _errors_JSONPointerCompileError_js__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(853);


const compile = referenceTokens => {
  if (!Array.isArray(referenceTokens)) {
    throw new TypeError('Reference tokens must be a list of strings or numbers');
  }
  try {
    if (referenceTokens.length === 0) {
      return '';
    }
    return `/${referenceTokens.map(referenceToken => {
      if (typeof referenceToken !== 'string' && typeof referenceToken !== 'number') {
        throw new TypeError('Reference token must be a string or number');
      }
      return (0,_escape_js__WEBPACK_IMPORTED_MODULE_1__["default"])(String(referenceToken));
    }).join('/')}`;
  } catch (error) {
    throw new _errors_JSONPointerCompileError_js__WEBPACK_IMPORTED_MODULE_0__["default"]('Unexpected error during JSON Pointer compilation', {
      cause: error,
      referenceTokens
    });
  }
};
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (compile);

/***/ }),

/***/ 71:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
/* harmony import */ var _JSONPointerEvaluateError_js__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(369);

class JSONPointerKeyError extends _JSONPointerEvaluateError_js__WEBPACK_IMPORTED_MODULE_0__["default"] {}
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (JSONPointerKeyError);

/***/ }),

/***/ 133:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
/* harmony import */ var _errors_JSONPointerError_js__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(628);

class EvaluationRealm {
  name = '';
  isArray(node) {
    throw new _errors_JSONPointerError_js__WEBPACK_IMPORTED_MODULE_0__["default"]('Realm.isArray(node) must be implemented in a subclass');
  }
  isObject(node) {
    throw new _errors_JSONPointerError_js__WEBPACK_IMPORTED_MODULE_0__["default"]('Realm.isObject(node) must be implemented in a subclass');
  }
  sizeOf(node) {
    throw new _errors_JSONPointerError_js__WEBPACK_IMPORTED_MODULE_0__["default"]('Realm.sizeOf(node) must be implemented in a subclass');
  }
  has(node, referenceToken) {
    throw new _errors_JSONPointerError_js__WEBPACK_IMPORTED_MODULE_0__["default"]('Realm.has(node) must be implemented in a subclass');
  }
  evaluate(node, referenceToken) {
    throw new _errors_JSONPointerError_js__WEBPACK_IMPORTED_MODULE_0__["default"]('Realm.evaluate(node) must be implemented in a subclass');
  }
}
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (EvaluationRealm);

/***/ }),

/***/ 142:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
/* harmony import */ var _CSTTranslator_js__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(979);
/* harmony import */ var _unescape_js__WEBPACK_IMPORTED_MODULE_1__ = __webpack_require__(415);


class ASTTranslator extends _CSTTranslator_js__WEBPACK_IMPORTED_MODULE_0__["default"] {
  getTree() {
    const {
      root
    } = super.getTree();
    return root.children.filter(({
      type
    }) => type === 'reference-token').map(({
      text
    }) => (0,_unescape_js__WEBPACK_IMPORTED_MODULE_1__["default"])(text));
  }
}
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (ASTTranslator);

/***/ }),

/***/ 348:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
/* harmony import */ var apg_lite__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(646);
/* harmony import */ var _grammar_js__WEBPACK_IMPORTED_MODULE_1__ = __webpack_require__(678);


const grammar = new _grammar_js__WEBPACK_IMPORTED_MODULE_1__["default"]();
const parser = new apg_lite__WEBPACK_IMPORTED_MODULE_0__.Parser();
const testArrayIndex = referenceToken => {
  if (typeof referenceToken !== 'string') return false;
  try {
    return parser.parse(grammar, 'array-index', referenceToken).success;
  } catch {
    return false;
  }
};
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (testArrayIndex);

/***/ }),

/***/ 353:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
class TraceBuilder {
  #trace;
  #path;
  #realm;
  constructor(trace, context = {}) {
    this.#trace = trace;
    this.#trace.steps = [];
    this.#trace.failed = false;
    this.#trace.failedAt = -1;
    this.#trace.message = `JSON Pointer "${context.jsonPointer}" was successfully evaluated against the provided value`;
    this.#trace.context = {
      ...context,
      realm: context.realm.name
    };
    this.#path = [];
    this.#realm = context.realm;
  }
  step({
    referenceToken,
    input,
    output,
    success = true,
    reason
  }) {
    const position = this.#path.length;
    this.#path.push(referenceToken);
    const step = {
      referenceToken,
      referenceTokenPosition: position,
      input,
      inputType: this.#realm.isObject(input) ? 'object' : this.#realm.isArray(input) ? 'array' : 'unrecognized',
      output,
      success
    };
    if (reason) {
      step.reason = reason;
    }
    this.#trace.steps.push(step);
    if (!success) {
      this.#trace.failed = true;
      this.#trace.failedAt = position;
      this.#trace.message = reason;
    }
  }
}
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (TraceBuilder);

/***/ }),

/***/ 369:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
/* harmony import */ var _JSONPointerError_js__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(628);

class JSONPointerEvaluateError extends _JSONPointerError_js__WEBPACK_IMPORTED_MODULE_0__["default"] {}
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (JSONPointerEvaluateError);

/***/ }),

/***/ 415:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
const unescape = referenceToken => {
  if (typeof referenceToken !== 'string') {
    throw new TypeError('Reference token must be a string');
  }
  return referenceToken.replace(/~1/g, '/').replace(/~0/g, '~');
};
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (unescape);

/***/ }),

/***/ 427:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
/* harmony import */ var _parse_index_js__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(583);
/* harmony import */ var _test_array_dash_js__WEBPACK_IMPORTED_MODULE_1__ = __webpack_require__(922);
/* harmony import */ var _test_array_index_js__WEBPACK_IMPORTED_MODULE_2__ = __webpack_require__(348);
/* harmony import */ var _trace_TraceBuilder_js__WEBPACK_IMPORTED_MODULE_8__ = __webpack_require__(353);
/* harmony import */ var _realms_json_index_js__WEBPACK_IMPORTED_MODULE_3__ = __webpack_require__(839);
/* harmony import */ var _errors_JSONPointerEvaluateError_js__WEBPACK_IMPORTED_MODULE_4__ = __webpack_require__(369);
/* harmony import */ var _errors_JSONPointerTypeError_js__WEBPACK_IMPORTED_MODULE_5__ = __webpack_require__(932);
/* harmony import */ var _errors_JSONPointerIndexError_js__WEBPACK_IMPORTED_MODULE_6__ = __webpack_require__(874);
/* harmony import */ var _errors_JSONPointerKeyError_js__WEBPACK_IMPORTED_MODULE_7__ = __webpack_require__(71);









const evaluate = (value, jsonPointer, {
  strictArrays = true,
  strictObjects = true,
  realm = new _realms_json_index_js__WEBPACK_IMPORTED_MODULE_3__["default"](),
  trace = true
} = {}) => {
  const {
    result: parseResult,
    tree: referenceTokens,
    trace: parseTrace
  } = (0,_parse_index_js__WEBPACK_IMPORTED_MODULE_0__["default"])(jsonPointer, {
    trace: !!trace
  });
  const tracer = typeof trace === 'object' && trace !== null ? new _trace_TraceBuilder_js__WEBPACK_IMPORTED_MODULE_8__["default"](trace, {
    jsonPointer,
    referenceTokens,
    strictArrays,
    strictObjects,
    realm,
    value
  }) : null;
  try {
    let output;
    if (!parseResult.success) {
      let message = `Invalid JSON Pointer: "${jsonPointer}". Syntax error at position ${parseResult.maxMatched}`;
      message += parseTrace ? `, expected ${parseTrace.inferExpectations()}` : '';
      throw new _errors_JSONPointerEvaluateError_js__WEBPACK_IMPORTED_MODULE_4__["default"](message, {
        jsonPointer,
        currentValue: value,
        realm: realm.name
      });
    }
    return referenceTokens.reduce((current, referenceToken, referenceTokenPosition) => {
      if (realm.isArray(current)) {
        if ((0,_test_array_dash_js__WEBPACK_IMPORTED_MODULE_1__["default"])(referenceToken)) {
          if (strictArrays) {
            throw new _errors_JSONPointerIndexError_js__WEBPACK_IMPORTED_MODULE_6__["default"](`Invalid array index "-" at position ${referenceTokenPosition} in "${jsonPointer}". The "-" token always refers to a nonexistent element during evaluation`, {
              jsonPointer,
              referenceTokens,
              referenceToken,
              referenceTokenPosition,
              currentValue: current,
              realm: realm.name
            });
          } else {
            output = realm.evaluate(current, String(realm.sizeOf(current)));
            tracer?.step({
              referenceToken,
              input: current,
              output
            });
            return output;
          }
        }
        if (!(0,_test_array_index_js__WEBPACK_IMPORTED_MODULE_2__["default"])(referenceToken)) {
          throw new _errors_JSONPointerIndexError_js__WEBPACK_IMPORTED_MODULE_6__["default"](`Invalid array index "${referenceToken}" at position ${referenceTokenPosition} in "${jsonPointer}": index MUST be "0", or digits without a leading "0"`, {
            jsonPointer,
            referenceTokens,
            referenceToken,
            referenceTokenPosition,
            currentValue: current,
            realm: realm.name
          });
        }
        const index = Number(referenceToken);
        if (!Number.isSafeInteger(index)) {
          throw new _errors_JSONPointerIndexError_js__WEBPACK_IMPORTED_MODULE_6__["default"](`Invalid array index "${referenceToken}" at position ${referenceTokenPosition} in "${jsonPointer}": index must be a safe integer`, {
            jsonPointer,
            referenceTokens,
            referenceToken,
            referenceTokenPosition,
            currentValue: current,
            realm: realm.name
          });
        }
        if (!realm.has(current, referenceToken) && strictArrays) {
          throw new _errors_JSONPointerIndexError_js__WEBPACK_IMPORTED_MODULE_6__["default"](`Invalid array index "${referenceToken}" at position ${referenceTokenPosition} in "${jsonPointer}": index not found in array`, {
            jsonPointer,
            referenceTokens,
            referenceToken,
            referenceTokenPosition,
            currentValue: current,
            realm: realm.name
          });
        }
        output = realm.evaluate(current, referenceToken);
        tracer?.step({
          referenceToken,
          input: current,
          output
        });
        return output;
      }
      if (realm.isObject(current)) {
        if (!realm.has(current, referenceToken) && strictObjects) {
          throw new _errors_JSONPointerKeyError_js__WEBPACK_IMPORTED_MODULE_7__["default"](`Invalid object key "${referenceToken}" at position ${referenceTokenPosition} in "${jsonPointer}": key not found in object`, {
            jsonPointer,
            referenceTokens,
            referenceToken,
            referenceTokenPosition,
            currentValue: current,
            realm: realm.name
          });
        }
        output = realm.evaluate(current, referenceToken);
        tracer?.step({
          referenceToken,
          input: current,
          output
        });
        return output;
      }
      throw new _errors_JSONPointerTypeError_js__WEBPACK_IMPORTED_MODULE_5__["default"](`Invalid reference token "${referenceToken}" at position ${referenceTokenPosition} in "${jsonPointer}": cannot be applied to a non-object/non-array value`, {
        jsonPointer,
        referenceTokens,
        referenceToken,
        referenceTokenPosition,
        currentValue: current,
        realm: realm.name
      });
    }, value);
  } catch (error) {
    tracer?.step({
      referenceToken: error.referenceToken,
      input: error.currentValue,
      success: false,
      reason: error.message
    });
    if (error instanceof _errors_JSONPointerEvaluateError_js__WEBPACK_IMPORTED_MODULE_4__["default"]) {
      throw error;
    }
    throw new _errors_JSONPointerEvaluateError_js__WEBPACK_IMPORTED_MODULE_4__["default"]('Unexpected error during JSON Pointer evaluation', {
      cause: error,
      jsonPointer,
      referenceTokens
    });
  }
};
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (evaluate);

/***/ }),

/***/ 525:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
/* harmony import */ var apg_lite__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(646);
/* harmony import */ var _grammar_js__WEBPACK_IMPORTED_MODULE_1__ = __webpack_require__(678);


const grammar = new _grammar_js__WEBPACK_IMPORTED_MODULE_1__["default"]();
const parser = new apg_lite__WEBPACK_IMPORTED_MODULE_0__.Parser();
const testArrayLocation = referenceToken => {
  if (typeof referenceToken !== 'string') return false;
  try {
    return parser.parse(grammar, 'array-location', referenceToken).success;
  } catch {
    return false;
  }
};
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (testArrayLocation);

/***/ }),

/***/ 533:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
/* harmony import */ var apg_lite__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(646);
/* harmony import */ var _grammar_js__WEBPACK_IMPORTED_MODULE_1__ = __webpack_require__(678);


const grammar = new _grammar_js__WEBPACK_IMPORTED_MODULE_1__["default"]();
const parser = new apg_lite__WEBPACK_IMPORTED_MODULE_0__.Parser();
const testReferenceToken = referenceToken => {
  if (typeof referenceToken !== 'string') return false;
  try {
    return parser.parse(grammar, 'reference-token', referenceToken).success;
  } catch {
    return false;
  }
};
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (testReferenceToken);

/***/ }),

/***/ 544:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
/* harmony import */ var _parse_index_js__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(583);

const testJSONPointer = jsonPointer => {
  try {
    const parseResult = (0,_parse_index_js__WEBPACK_IMPORTED_MODULE_0__["default"])(jsonPointer, {
      translator: null
    });
    return parseResult.result.success;
  } catch {
    return false;
  }
};
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (testJSONPointer);

/***/ }),

/***/ 583:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
/* harmony import */ var apg_lite__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(646);
/* harmony import */ var _grammar_js__WEBPACK_IMPORTED_MODULE_4__ = __webpack_require__(678);
/* harmony import */ var _errors_JSONPointerParseError_js__WEBPACK_IMPORTED_MODULE_1__ = __webpack_require__(819);
/* harmony import */ var _translators_ASTTranslator_js__WEBPACK_IMPORTED_MODULE_2__ = __webpack_require__(142);
/* harmony import */ var _trace_Trace_js__WEBPACK_IMPORTED_MODULE_3__ = __webpack_require__(992);





const grammar = new _grammar_js__WEBPACK_IMPORTED_MODULE_4__["default"]();
const parse = (jsonPointer, {
  translator = new _translators_ASTTranslator_js__WEBPACK_IMPORTED_MODULE_2__["default"](),
  stats = false,
  trace = false
} = {}) => {
  if (typeof jsonPointer !== 'string') {
    throw new TypeError('JSON Pointer must be a string');
  }
  try {
    const parser = new apg_lite__WEBPACK_IMPORTED_MODULE_0__.Parser();
    if (translator) parser.ast = translator;
    if (stats) parser.stats = new apg_lite__WEBPACK_IMPORTED_MODULE_0__.Stats();
    if (trace) parser.trace = new _trace_Trace_js__WEBPACK_IMPORTED_MODULE_3__["default"]();
    const result = parser.parse(grammar, 'json-pointer', jsonPointer);
    return {
      result,
      tree: result.success && translator ? parser.ast.getTree() : undefined,
      stats: parser.stats,
      trace: parser.trace
    };
  } catch (error) {
    throw new _errors_JSONPointerParseError_js__WEBPACK_IMPORTED_MODULE_1__["default"]('Unexpected error during JSON Pointer parsing', {
      cause: error,
      jsonPointer
    });
  }
};
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (parse);

/***/ }),

/***/ 628:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
class JSONPointerError extends Error {
  constructor(message, options = undefined) {
    super(message, options);
    this.name = this.constructor.name;
    if (typeof message === 'string') {
      this.message = message;
    }
    if (typeof Error.captureStackTrace === 'function') {
      Error.captureStackTrace(this, this.constructor);
    } else {
      this.stack = new Error(message).stack;
    }

    /**
     * This needs to stay here until our minimum supported version of Node.js is >= 16.9.0.
     * Node.js is >= 16.9.0 supports error causes natively.
     */
    if (options != null && typeof options === 'object' && Object.prototype.hasOwnProperty.call(options, 'cause') && !('cause' in this)) {
      const {
        cause
      } = options;
      this.cause = cause;
      if (cause instanceof Error && 'stack' in cause) {
        this.stack = `${this.stack}\nCAUSE: ${cause.stack}`;
      }
    }

    /**
     * Allows to assign arbitrary properties to the error object.
     */
    if (options != null && typeof options === 'object') {
      const {
        cause,
        ...causelessOptions
      } = options;
      Object.assign(this, causelessOptions);
    }
  }
}
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (JSONPointerError);

/***/ }),

/***/ 632:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
/* harmony import */ var _EvaluationRealm_js__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(133);
/* harmony import */ var _errors_JSONPointerEvaluateError_js__WEBPACK_IMPORTED_MODULE_1__ = __webpack_require__(369);


class CompositeEvaluationRealm extends _EvaluationRealm_js__WEBPACK_IMPORTED_MODULE_0__["default"] {
  name = 'composite';
  realms = [];
  constructor(realms) {
    super();
    this.realms = realms;
  }
  isArray(node) {
    return this.#findRealm(node).isArray(node);
  }
  isObject(node) {
    return this.#findRealm(node).isObject(node);
  }
  sizeOf(node) {
    return this.#findRealm(node).sizeOf(node);
  }
  has(node, referenceToken) {
    return this.#findRealm(node).has(node, referenceToken);
  }
  evaluate(node, referenceToken) {
    return this.#findRealm(node).evaluate(node, referenceToken);
  }
  #findRealm(node) {
    for (const realm of this.realms) {
      if (realm.isArray(node) || realm.isObject(node)) {
        return realm;
      }
    }
    throw new _errors_JSONPointerEvaluateError_js__WEBPACK_IMPORTED_MODULE_1__["default"]('No suitable evaluation realm found for value', {
      currentValue: node
    });
  }
}
const compose = (...realms) => new CompositeEvaluationRealm(realms);
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (compose);

/***/ }),

/***/ 646:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   Ast: () => (/* binding */ Ast),
/* harmony export */   Parser: () => (/* binding */ Parser),
/* harmony export */   Stats: () => (/* binding */ Stats),
/* harmony export */   Trace: () => (/* binding */ Trace),
/* harmony export */   identifiers: () => (/* binding */ identifiers),
/* harmony export */   utilities: () => (/* binding */ utilities)
/* harmony export */ });
/*  *************************************************************************************
 *   copyright: Copyright (c) 2023 Lowell D. Thomas, all rights reserved
 *     license: BSD-2-Clause (https://opensource.org/licenses/BSD-2-Clause)
 *
 *    Redistribution and use in source and binary forms, with or without
 *    modification, are permitted provided that the following conditions are met:
 *
 *    1. Redistributions of source code must retain the above copyright notice, this
 *       list of conditions and the following disclaimer.
 *
 *    2. Redistributions in binary form must reproduce the above copyright notice,
 *       this list of conditions and the following disclaimer in the documentation
 *       and/or other materials provided with the distribution.
 *
 *    THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
 *    AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 *    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 *    DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
 *    FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 *    DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 *    SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
 *    CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
 *    OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
 *    OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 *
 *   ********************************************************************************* */


const Parser = function fnparser() {
  const id = identifiers;
  const utils = utilities;
  const p = this;
  const thisFileName = 'parser.js: Parser(): ';
  const systemData = function systemData() {
    this.state = id.ACTIVE;
    this.phraseLength = 0;
    this.refresh = () => {
      this.state = id.ACTIVE;
      this.phraseLength = 0;
    };
  };
  p.ast = undefined;
  p.stats = undefined;
  p.trace = undefined;
  p.callbacks = [];
  let lookAhead = 0;
  let treeDepth = 0;
  let maxTreeDepth = 0;
  let nodeHits = 0;
  let maxMatched = 0;
  let rules = undefined;
  let udts = undefined;
  let opcodes = undefined;
  let chars = undefined;
  let sysData = new systemData();
  let ruleCallbacks = undefined;
  let udtCallbacks = undefined;
  let userData = undefined;
  const clear = () => {
    lookAhead = 0;
    treeDepth = 0;
    maxTreeDepth = 0;
    nodeHits = 0;
    maxMatched = 0;
    rules = undefined;
    udts = undefined;
    opcodes = undefined;
    chars = undefined;
    sysData.refresh();
    ruleCallbacks = undefined;
    udtCallbacks = undefined;
    userData = undefined;
  };

  const initializeCallbacks = () => {
    const functionName = `${thisFileName}initializeCallbacks(): `;
    let i;
    ruleCallbacks = [];
    udtCallbacks = [];
    for (i = 0; i < rules.length; i += 1) {
      ruleCallbacks[i] = undefined;
    }
    for (i = 0; i < udts.length; i += 1) {
      udtCallbacks[i] = undefined;
    }
    let func;
    const list = [];
    for (i = 0; i < rules.length; i += 1) {
      list.push(rules[i].lower);
    }
    for (i = 0; i < udts.length; i += 1) {
      list.push(udts[i].lower);
    }
    for (const index in p.callbacks) {
      if (p.callbacks.hasOwnProperty(index)) {
        i = list.indexOf(index.toLowerCase());
        if (i < 0) {
          throw new Error(`${functionName}syntax callback '${index}' not a rule or udt name`);
        }
        func = p.callbacks[index] ? p.callbacks[index] : undefined;
        if (typeof func === 'function' || func === undefined) {
          if (i < rules.length) {
            ruleCallbacks[i] = func;
          } else {
            udtCallbacks[i - rules.length] = func;
          }
        } else {
          throw new Error(`${functionName}syntax callback[${index}] must be function reference or falsy)`);
        }
      }
    }
  };

  p.parse = (grammar, startName, inputString, callbackData) => {
    const functionName = `${thisFileName}parse(): `;
    clear();
    chars = utils.stringToChars(inputString);
    rules = grammar.rules;
    udts = grammar.udts;
    const lower = startName.toLowerCase();
    let startIndex = undefined;
    for (const i in rules) {
      if (rules.hasOwnProperty(i)) {
        if (lower === rules[i].lower) {
          startIndex = rules[i].index;
          break;
        }
      }
    }
    if (startIndex === undefined) {
      throw new Error(`${functionName}start rule name '${startRule}' not recognized`);
    }
    initializeCallbacks();
    if (p.trace) {
      p.trace.init(rules, udts, chars);
    }
    if (p.stats) {
      p.stats.init(rules, udts);
    }
    if (p.ast) {
      p.ast.init(rules, udts, chars);
    }
    userData = callbackData;
    /* create a dummy opcode for the start rule */
    opcodes = [
      {
        type: id.RNM,
        index: startIndex,
      },
    ];
    /* execute the start rule */
    opExecute(0, 0);
    opcodes = undefined;
    /* test and return the sysData */
    let success = false;
    switch (sysData.state) {
      case id.ACTIVE:
        throw new Error(`${functionName}final state should never be 'ACTIVE'`);
      case id.NOMATCH:
        success = false;
        break;
      case id.EMPTY:
      case id.MATCH:
        if (sysData.phraseLength === chars.length) {
          success = true;
        } else {
          success = false;
        }
        break;
      default:
        throw new Error('unrecognized state');
    }
    return {
      success,
      state: sysData.state,
      stateName: id.idName(sysData.state),
      length: chars.length,
      matched: sysData.phraseLength,
      maxMatched,
      maxTreeDepth,
      nodeHits,
    };
  };
  // The `ALT` operator.<br>
  // Executes its child nodes, from left to right, until it finds a match.
  // Fails if *all* of its child nodes fail.
  const opALT = (opIndex, phraseIndex) => {
    const op = opcodes[opIndex];
    for (let i = 0; i < op.children.length; i += 1) {
      opExecute(op.children[i], phraseIndex);
      if (sysData.state !== id.NOMATCH) {
        break;
      }
    }
  };
  // The `CAT` operator.<br>
  // Executes all of its child nodes, from left to right,
  // concatenating the matched phrases.
  // Fails if *any* child nodes fail.
  const opCAT = (opIndex, phraseIndex) => {
    let success;
    let astLength;
    let catCharIndex;
    let catPhrase;
    const op = opcodes[opIndex];
    if (p.ast) {
      astLength = p.ast.getLength();
    }
    success = true;
    catCharIndex = phraseIndex;
    catPhrase = 0;
    for (let i = 0; i < op.children.length; i += 1) {
      opExecute(op.children[i], catCharIndex);
      if (sysData.state === id.NOMATCH) {
        success = false;
        break;
      } else {
        catCharIndex += sysData.phraseLength;
        catPhrase += sysData.phraseLength;
      }
    }
    if (success) {
      sysData.state = catPhrase === 0 ? id.EMPTY : id.MATCH;
      sysData.phraseLength = catPhrase;
    } else {
      sysData.state = id.NOMATCH;
      sysData.phraseLength = 0;
      if (p.ast) {
        p.ast.setLength(astLength);
      }
    }
  };
  // The `REP` operator.<br>
  // Repeatedly executes its single child node,
  // concatenating each of the matched phrases found.
  // The number of repetitions executed and its final sysData depends
  // on its `min` & `max` repetition values.
  const opREP = (opIndex, phraseIndex) => {
    let astLength;
    let repCharIndex;
    let repPhrase;
    let repCount;
    const op = opcodes[opIndex];
    if (op.max === 0) {
      // this is an empty-string acceptor
      // deprecated: use the TLS empty string operator, "", instead
      sysData.state = id.EMPTY;
      sysData.phraseLength = 0;
      return;
    }
    repCharIndex = phraseIndex;
    repPhrase = 0;
    repCount = 0;
    if (p.ast) {
      astLength = p.ast.getLength();
    }
    while (1) {
      if (repCharIndex >= chars.length) {
        /* exit on end of input string */
        break;
      }
      opExecute(opIndex + 1, repCharIndex);
      if (sysData.state === id.NOMATCH) {
        /* always end if the child node fails */
        break;
      }
      if (sysData.state === id.EMPTY) {
        /* REP always succeeds when the child node returns an empty phrase */
        /* this may not seem obvious, but that's the way it works out */
        break;
      }
      repCount += 1;
      repPhrase += sysData.phraseLength;
      repCharIndex += sysData.phraseLength;
      if (repCount === op.max) {
        /* end on maxed out reps */
        break;
      }
    }
    /* evaluate the match count according to the min, max values */
    if (sysData.state === id.EMPTY) {
      sysData.state = repPhrase === 0 ? id.EMPTY : id.MATCH;
      sysData.phraseLength = repPhrase;
    } else if (repCount >= op.min) {
      sysData.state = repPhrase === 0 ? id.EMPTY : id.MATCH;
      sysData.phraseLength = repPhrase;
    } else {
      sysData.state = id.NOMATCH;
      sysData.phraseLength = 0;
      if (p.ast) {
        p.ast.setLength(astLength);
      }
    }
  };
  // Validate the callback function's returned sysData values.
  // It's the user's responsibility to get them right
  // but `RNM` fails if not.
  const validateRnmCallbackResult = (rule, sysData, charsLeft, down) => {
    if (sysData.phraseLength > charsLeft) {
      let str = `${thisFileName}opRNM(${rule.name}): callback function error: `;
      str += `sysData.phraseLength: ${sysData.phraseLength}`;
      str += ` must be <= remaining chars: ${charsLeft}`;
      throw new Error(str);
    }
    switch (sysData.state) {
      case id.ACTIVE:
        if (!down) {
          throw new Error(
            `${thisFileName}opRNM(${rule.name}): callback function return error. ACTIVE state not allowed.`
          );
        }
        break;
      case id.EMPTY:
        sysData.phraseLength = 0;
        break;
      case id.MATCH:
        if (sysData.phraseLength === 0) {
          sysData.state = id.EMPTY;
        }
        break;
      case id.NOMATCH:
        sysData.phraseLength = 0;
        break;
      default:
        throw new Error(
          `${thisFileName}opRNM(${rule.name}): callback function return error. Unrecognized return state: ${sysData.state}`
        );
    }
  };
  // The `RNM` operator.<br>
  // This operator will acts as a root node for a parse tree branch below and
  // returns the matched phrase to its parent.
  // However, its larger responsibility is handling user-defined callback functions and `AST` nodes.
  // Note that the `AST` is a separate object, but `RNM` calls its functions to create its nodes.
  const opRNM = (opIndex, phraseIndex) => {
    let astLength;
    let astDefined;
    let savedOpcodes;
    const op = opcodes[opIndex];
    const rule = rules[op.index];
    const callback = ruleCallbacks[rule.index];
    /* ignore AST in look ahead (AND or NOT operator above) */
    if (!lookAhead) {
      astDefined = p.ast && p.ast.ruleDefined(op.index);
      if (astDefined) {
        astLength = p.ast.getLength();
        p.ast.down(op.index, rules[op.index].name);
      }
    }
    if (callback) {
      /* call user's callback going down the parse tree*/
      const charsLeft = chars.length - phraseIndex;
      callback(sysData, chars, phraseIndex, userData);
      validateRnmCallbackResult(rule, sysData, charsLeft, true);
      if (sysData.state === id.ACTIVE) {
        savedOpcodes = opcodes;
        opcodes = rule.opcodes;
        opExecute(0, phraseIndex);
        opcodes = savedOpcodes;
        /* call user's callback going up the parse tree*/
        callback(sysData, chars, phraseIndex, userData);
        validateRnmCallbackResult(rule, sysData, charsLeft, false);
      } /* implied else clause: just accept the callback sysData - RNM acting as UDT */
    } else {
      /* no callback - just execute the rule */
      savedOpcodes = opcodes;
      opcodes = rule.opcodes;
      opExecute(0, phraseIndex, sysData);
      opcodes = savedOpcodes;
    }
    if (!lookAhead) {
      /* end AST */
      if (astDefined) {
        if (sysData.state === id.NOMATCH) {
          p.ast.setLength(astLength);
        } else {
          p.ast.up(op.index, rule.name, phraseIndex, sysData.phraseLength);
        }
      }
    }
  };
  // The `TRG` operator.<br>
  // Succeeds if the single first character of the phrase is
  // within the `min - max` range.
  const opTRG = (opIndex, phraseIndex) => {
    const op = opcodes[opIndex];
    sysData.state = id.NOMATCH;
    if (phraseIndex < chars.length) {
      if (op.min <= chars[phraseIndex] && chars[phraseIndex] <= op.max) {
        sysData.state = id.MATCH;
        sysData.phraseLength = 1;
      }
    }
  };
  // The `TBS` operator.<br>
  // Matches its pre-defined phrase against the input string.
  // All characters must match exactly.
  // Case-sensitive literal strings (`'string'` & `%s"string"`) are translated to `TBS`
  // operators by `apg`.
  // Phrase length of zero is not allowed.
  // Empty phrases can only be defined with `TLS` operators.
  const opTBS = (opIndex, phraseIndex) => {
    const op = opcodes[opIndex];
    const len = op.string.length;
    sysData.state = id.NOMATCH;
    if (phraseIndex + len <= chars.length) {
      for (let i = 0; i < len; i += 1) {
        if (chars[phraseIndex + i] !== op.string[i]) {
          return;
        }
      }
      sysData.state = id.MATCH;
      sysData.phraseLength = len;
    } /* implied else NOMATCH */
  };
  // The `TLS` operator.<br>
  // Matches its pre-defined phrase against the input string.
  // A case-insensitive match is attempted for ASCII alphbetical characters.
  // `TLS` is the only operator that explicitly allows empty phrases.
  // `apg` will fail for empty `TBS`, case-sensitive strings (`''`) or
  // zero repetitions (`0*0RuleName` or `0RuleName`).
  const opTLS = (opIndex, phraseIndex) => {
    let code;
    const op = opcodes[opIndex];
    sysData.state = id.NOMATCH;
    const len = op.string.length;
    if (len === 0) {
      /* EMPTY match allowed for TLS */
      sysData.state = id.EMPTY;
      return;
    }
    if (phraseIndex + len <= chars.length) {
      for (let i = 0; i < len; i += 1) {
        code = chars[phraseIndex + i];
        if (code >= 65 && code <= 90) {
          code += 32;
        }
        if (code !== op.string[i]) {
          return;
        }
      }
      sysData.state = id.MATCH;
      sysData.phraseLength = len;
    } /* implied else NOMATCH */
  };
  // Validate the callback function's returned sysData values.
  // It's the user's responsibility to get it right but `UDT` fails if not.
  const validateUdtCallbackResult = (udt, sysData, charsLeft) => {
    if (sysData.phraseLength > charsLeft) {
      let str = `${thisFileName}opUDT(${udt.name}): callback function error: `;
      str += `sysData.phraseLength: ${sysData.phraseLength}`;
      str += ` must be <= remaining chars: ${charsLeft}`;
      throw new Error(str);
    }
    switch (sysData.state) {
      case id.ACTIVE:
        throw new Error(`${thisFileName}opUDT(${udt.name}) ACTIVE state return not allowed.`);
      case id.EMPTY:
        if (udt.empty) {
          sysData.phraseLength = 0;
        } else {
          throw new Error(`${thisFileName}opUDT(${udt.name}) may not return EMPTY.`);
        }
        break;
      case id.MATCH:
        if (sysData.phraseLength === 0) {
          if (udt.empty) {
            sysData.state = id.EMPTY;
          } else {
            throw new Error(`${thisFileName}opUDT(${udt.name}) may not return EMPTY.`);
          }
        }
        break;
      case id.NOMATCH:
        sysData.phraseLength = 0;
        break;
      default:
        throw new Error(
          `${thisFileName}opUDT(${udt.name}): callback function return error. Unrecognized return state: ${sysData.state}`
        );
    }
  };
  // The `UDT` operator.<br>
  // Simply calls the user's callback function, but operates like `RNM` with regard to the `AST`
  // and back referencing.
  // There is some ambiguity here. `UDT`s act as terminals for phrase recognition but as named rules
  // for `AST` nodes and back referencing.
  // See [`ast.js`](./ast.html) for usage.
  const opUDT = (opIndex, phraseIndex) => {
    let astLength;
    let astIndex;
    let astDefined;
    const op = opcodes[opIndex];
    const udt = udts[op.index];
    sysData.UdtIndex = udt.index;
    /* ignore AST in look ahead */
    if (!lookAhead) {
      astDefined = p.ast && p.ast.udtDefined(op.index);
      if (astDefined) {
        astIndex = rules.length + op.index;
        astLength = p.ast.getLength();
        p.ast.down(astIndex, udt.name);
      }
    }
    /* call the UDT */
    const charsLeft = chars.length - phraseIndex;
    udtCallbacks[op.index](sysData, chars, phraseIndex, userData);
    validateUdtCallbackResult(udt, sysData, charsLeft);
    if (!lookAhead) {
      /* end AST */
      if (astDefined) {
        if (sysData.state === id.NOMATCH) {
          p.ast.setLength(astLength);
        } else {
          p.ast.up(astIndex, udt.name, phraseIndex, sysData.phraseLength);
        }
      }
    }
  };
  // The `AND` operator.<br>
  // This is the positive `look ahead` operator.
  // Executes its single child node, returning the EMPTY state
  // if it succeedsand NOMATCH if it fails.
  // *Always* backtracks on any matched phrase and returns EMPTY on success.
  const opAND = (opIndex, phraseIndex) => {
    lookAhead += 1;
    opExecute(opIndex + 1, phraseIndex);
    lookAhead -= 1;
    sysData.phraseLength = 0;
    switch (sysData.state) {
      case id.EMPTY:
        sysData.state = id.EMPTY;
        break;
      case id.MATCH:
        sysData.state = id.EMPTY;
        break;
      case id.NOMATCH:
        sysData.state = id.NOMATCH;
        break;
      default:
        throw new Error(`opAND: invalid state ${sysData.state}`);
    }
  };
  // The `NOT` operator.<br>
  // This is the negative `look ahead` operator.
  // Executes its single child node, returning the EMPTY state
  // if it *fails* and NOMATCH if it succeeds.
  // *Always* backtracks on any matched phrase and returns EMPTY
  // on success (failure of its child node).
  const opNOT = (opIndex, phraseIndex) => {
    lookAhead += 1;
    opExecute(opIndex + 1, phraseIndex);
    lookAhead -= 1;
    sysData.phraseLength = 0;
    switch (sysData.state) {
      case id.EMPTY:
      case id.MATCH:
        sysData.state = id.NOMATCH;
        break;
      case id.NOMATCH:
        sysData.state = id.EMPTY;
        break;
      default:
        throw new Error(`opNOT: invalid state ${sysData.state}`);
    }
  };

  const opExecute = (opIndex, phraseIndex) => {
    const functionName = `${thisFileName}opExecute(): `;
    const op = opcodes[opIndex];
    nodeHits += 1;
    if (treeDepth > maxTreeDepth) {
      maxTreeDepth = treeDepth;
    }
    treeDepth += 1;
    sysData.refresh();
    if (p.trace) {
      p.trace.down(op, phraseIndex);
    }
    switch (op.type) {
      case id.ALT:
        opALT(opIndex, phraseIndex);
        break;
      case id.CAT:
        opCAT(opIndex, phraseIndex);
        break;
      case id.REP:
        opREP(opIndex, phraseIndex);
        break;
      case id.RNM:
        opRNM(opIndex, phraseIndex);
        break;
      case id.TRG:
        opTRG(opIndex, phraseIndex);
        break;
      case id.TBS:
        opTBS(opIndex, phraseIndex);
        break;
      case id.TLS:
        opTLS(opIndex, phraseIndex);
        break;
      case id.UDT:
        opUDT(opIndex, phraseIndex);
        break;
      case id.AND:
        opAND(opIndex, phraseIndex);
        break;
      case id.NOT:
        opNOT(opIndex, phraseIndex);
        break;
      default:
        throw new Error(`${functionName}unrecognized operator`);
    }
    if (!lookAhead) {
      if (phraseIndex + sysData.phraseLength > maxMatched) {
        maxMatched = phraseIndex + sysData.phraseLength;
      }
    }
    if (p.stats) {
      p.stats.collect(op, sysData);
    }
    if (p.trace) {
      p.trace.up(op, sysData.state, phraseIndex, sysData.phraseLength);
    }
    treeDepth -= 1;
  };
};

const Ast = function fnast() {
  const thisFileName = 'parser.js: Ast()): ';
  const id = identifiers;
  const utils = utilities;
  const a = this;
  let rules = undefined;
  let udts = undefined;
  let chars = undefined;
  let nodeCount = 0;
  const nodeCallbacks = [];
  const stack = [];
  const records = [];
  a.callbacks = [];
  /* called by the parser to initialize the AST with the rules, UDTs and the input characters */
  a.init = (rulesIn, udtsIn, charsIn) => {
    stack.length = 0;
    records.length = 0;
    nodeCount = 0;
    rules = rulesIn;
    udts = udtsIn;
    chars = charsIn;
    let i;
    const list = [];
    for (i = 0; i < rules.length; i += 1) {
      list.push(rules[i].lower);
    }
    for (i = 0; i < udts.length; i += 1) {
      list.push(udts[i].lower);
    }
    nodeCount = rules.length + udts.length;
    for (i = 0; i < nodeCount; i += 1) {
      nodeCallbacks[i] = undefined;
    }
    for (const index in a.callbacks) {
      if (a.callbacks.hasOwnProperty(index)) {
        const lower = index.toLowerCase();
        i = list.indexOf(lower);
        if (i < 0) {
          throw new Error(`${thisFileName}init: node '${index}' not a rule or udt name`);
        }
        nodeCallbacks[i] = a.callbacks[index];
      }
    }
  };
  /* AST node rule callbacks - called by the parser's `RNM` operator */
  a.ruleDefined = (index) => !!nodeCallbacks[index];
  /* AST node UDT callbacks - called by the parser's `UDT` operator */
  a.udtDefined = (index) => !!nodeCallbacks[rules.length + index];
  /* called by the parser's `RNM` & `UDT` operators
     builds a record for the downward traversal of the node */
  a.down = (callbackIndex, name) => {
    const thisIndex = records.length;
    stack.push(thisIndex);
    records.push({
      name,
      thisIndex,
      thatIndex: undefined,
      state: id.SEM_PRE,
      callbackIndex,
      phraseIndex: undefined,
      phraseLength: undefined,
      stack: stack.length,
    });
    return thisIndex;
  };
  /* called by the parser's `RNM` & `UDT` operators */
  /* builds a record for the upward traversal of the node */
  a.up = (callbackIndex, name, phraseIndex, phraseLength) => {
    const thisIndex = records.length;
    const thatIndex = stack.pop();
    records.push({
      name,
      thisIndex,
      thatIndex,
      state: id.SEM_POST,
      callbackIndex,
      phraseIndex,
      phraseLength,
      stack: stack.length,
    });
    records[thatIndex].thatIndex = thisIndex;
    records[thatIndex].phraseIndex = phraseIndex;
    records[thatIndex].phraseLength = phraseLength;
    return thisIndex;
  };
  // Called by the user to translate the AST.
  // Translate means to associate or apply some semantic action to the
  // phrases that were syntactically matched to the AST nodes according
  // to the defining grammar.
  // ```
  // data - optional user-defined data
  //        passed to the callback functions by the translator
  // ```
  a.translate = (data) => {
    let ret;
    let callback;
    let record;
    for (let i = 0; i < records.length; i += 1) {
      record = records[i];
      callback = nodeCallbacks[record.callbackIndex];
      if (callback) {
        if (record.state === id.SEM_PRE) {
          callback(id.SEM_PRE, chars, record.phraseIndex, record.phraseLength, data);
        } else if (callback) {
          callback(id.SEM_POST, chars, record.phraseIndex, record.phraseLength, data);
        }
      }
    }
  };
  /* called by the parser to reset the length of the records array */
  /* necessary on backtracking */
  a.setLength = (length) => {
    records.length = length;
    if (length > 0) {
      stack.length = records[length - 1].stack;
    } else {
      stack.length = 0;
    }
  };
  /* called by the parser to get the length of the records array */
  a.getLength = () => records.length;
  /* helper for XML display */
  function indent(n) {
    let ret = '';
    while (n-- > 0) {
      ret += ' ';
    }
    return ret;
  }
  // Generate an `XML` version of the AST.
  // Useful if you want to use a special or favorite XML parser to translate the
  // AST. Node data are JavaScript strings.
  a.toXml = () => {
    let xml = '';
    let depth = 0;
    xml += '<?xml version="1.0" encoding="utf-8"?>\n';
    xml += `<root nodes="${records.length / 2}" characters="${chars.length}">\n`;
    xml += `<!-- input string -->\n`;
    xml += indent(depth + 2);
    xml += utils.charsToString(chars);
    xml += '\n';
    records.forEach((rec) => {
      if (rec.state === id.SEM_PRE) {
        depth += 1;
        xml += indent(depth);
        xml += `<node name="${rec.name}" index="${rec.phraseIndex}" length="${rec.phraseLength}">\n`;
        xml += indent(depth + 2);
        xml += utils.charsToString(chars, rec.phraseIndex, rec.phraseLength);
        xml += '\n';
      } else {
        xml += indent(depth);
        xml += `</node><!-- name="${rec.name}" -->\n`;
        depth -= 1;
      }
    });

    xml += '</root>\n';
    return xml;
  };
};

const Trace = function fntrace() {
  const id = identifiers;
  const utils = utilities;
  const thisFile = 'parser.js: Trace(): ';
  let chars = undefined;
  let rules = undefined;
  let udts = undefined;
  let out = '';
  let treeDepth = 0;
  const MAX_PHRASE = 100;
  const t = this;
  const indent = (n) => {
    let ret = '';
    let count = 0;
    if (n >= 0) {
      while (n--) {
        count += 1;
        if (count === 5) {
          ret += '|';
          count = 0;
        } else {
          ret += '.';
        }
      }
    }
    return ret;
  };
  t.init = (r, u, c) => {
    rules = r;
    udts = u;
    chars = c;
  };
  const opName = (op) => {
    let name;
    switch (op.type) {
      case id.ALT:
        name = 'ALT';
        break;
      case id.CAT:
        name = 'CAT';
        break;
      case id.REP:
        if (op.max === Infinity) {
          name = `REP(${op.min},inf)`;
        } else {
          name = `REP(${op.min},${op.max})`;
        }
        break;
      case id.RNM:
        name = `RNM(${rules[op.index].name})`;
        break;
      case id.TRG:
        name = `TRG(${op.min},${op.max})`;
        break;
      case id.TBS:
        if (op.string.length > 6) {
          name = `TBS(${utils.charsToString(op.string, 0, 3)}...)`;
        } else {
          name = `TBS(${utils.charsToString(op.string, 0, 6)})`;
        }
        break;
      case id.TLS:
        if (op.string.length > 6) {
          name = `TLS(${utils.charsToString(op.string, 0, 3)}...)`;
        } else {
          name = `TLS(${utils.charsToString(op.string, 0, 6)})`;
        }
        break;
      case id.UDT:
        name = `UDT(${udts[op.index].name})`;
        break;
      case id.AND:
        name = 'AND';
        break;
      case id.NOT:
        name = 'NOT';
        break;
      default:
        throw new Error(`${thisFile}Trace: opName: unrecognized opcode`);
    }
    return name;
  };
  t.down = (op, offset) => {
    const lead = indent(treeDepth);
    const len = Math.min(MAX_PHRASE, chars.length - offset);
    let phrase = utils.charsToString(chars, offset, len);
    if (len < chars.length - offset) {
      phrase += '...';
    }
    phrase = `${lead}|-|[${opName(op)}]${phrase}\n`;
    out += phrase;
    treeDepth += 1;
  };
  t.up = (op, state, offset, phraseLength) => {
    const thisFunc = `${thisFile}trace.up: `;
    treeDepth -= 1;
    const lead = indent(treeDepth);
    let len;
    let phrase;
    let st;
    switch (state) {
      case id.EMPTY:
        st = '|E|';
        phrase = `''`;
        break;
      case id.MATCH:
        st = '|M|';
        len = Math.min(MAX_PHRASE, phraseLength);
        if (len < phraseLength) {
          phrase = `'${utils.charsToString(chars, offset, len)}...'`;
        } else {
          phrase = `'${utils.charsToString(chars, offset, len)}'`;
        }
        break;
      case id.NOMATCH:
        st = '|N|';
        phrase = '';
        break;
      default:
        throw new Error(`${thisFunc} unrecognized state`);
    }
    phrase = `${lead}${st}[${opName(op)}]${phrase}\n`;
    out += phrase;
  };
  t.displayTrace = () => out;
};

const Stats = function fnstats() {
  const id = identifiers;
  const thisFileName = 'parser.js: Stats(): ';
  let rules;
  let udts;
  let totals;
  const stats = [];
  const ruleStats = [];
  const udtStats = [];
  /* called by parser to initialize the stats */
  this.init = (r, u) => {
    rules = r;
    udts = u;
    clear();
  };
  /* This function is the main interaction with the parser. */
  /* The parser calls it after each node has been traversed. */
  this.collect = (op, sys) => {
    incStat(totals, sys.state, sys.phraseLength);
    incStat(stats[op.type], sys.state, sys.phraseLength);
    if (op.type === id.RNM) {
      incStat(ruleStats[op.index], sys.state, sys.phraseLength);
    }
    if (op.type === id.UDT) {
      incStat(udtStats[op.index], sys.state, sys.phraseLength);
    }
  };
  this.displayStats = () => {
    let out = '';
    const totals = {
      match: 0,
      empty: 0,
      nomatch: 0,
      total: 0,
    };
    const displayRow = (op, m, e, n, t) => {
      totals.match += m;
      totals.empty += e;
      totals.nomatch += n;
      totals.total += t;
      const mm = normalize(m);
      const ee = normalize(e);
      const nn = normalize(n);
      const tt = normalize(t);
      return `${op} | ${mm} | ${ee} | ${nn} | ${tt} |\n`;
    };
    out += '          OPERATOR STATS\n';
    out += '      |   MATCH |   EMPTY | NOMATCH |   TOTAL |\n';
    out += displayRow('  ALT', stats[id.ALT].match, stats[id.ALT].empty, stats[id.ALT].nomatch, stats[id.ALT].total);
    out += displayRow('  CAT', stats[id.CAT].match, stats[id.CAT].empty, stats[id.CAT].nomatch, stats[id.CAT].total);
    out += displayRow('  REP', stats[id.REP].match, stats[id.REP].empty, stats[id.REP].nomatch, stats[id.REP].total);
    out += displayRow('  RNM', stats[id.RNM].match, stats[id.RNM].empty, stats[id.RNM].nomatch, stats[id.RNM].total);
    out += displayRow('  TRG', stats[id.TRG].match, stats[id.TRG].empty, stats[id.TRG].nomatch, stats[id.TRG].total);
    out += displayRow('  TBS', stats[id.TBS].match, stats[id.TBS].empty, stats[id.TBS].nomatch, stats[id.TBS].total);
    out += displayRow('  TLS', stats[id.TLS].match, stats[id.TLS].empty, stats[id.TLS].nomatch, stats[id.TLS].total);
    out += displayRow('  UDT', stats[id.UDT].match, stats[id.UDT].empty, stats[id.UDT].nomatch, stats[id.UDT].total);
    out += displayRow('  AND', stats[id.AND].match, stats[id.AND].empty, stats[id.AND].nomatch, stats[id.AND].total);
    out += displayRow('  NOT', stats[id.NOT].match, stats[id.NOT].empty, stats[id.NOT].nomatch, stats[id.NOT].total);
    out += displayRow('TOTAL', totals.match, totals.empty, totals.nomatch, totals.total);
    return out;
  };
  /*
  Display rule/udt
  */
  this.displayHits = (type) => {
    let out = '';
    const displayRow = (m, e, n, t, name) => {
      totals.match += m;
      totals.empty += e;
      totals.nomatch += n;
      totals.total += t;
      const mm = normalize(m);
      const ee = normalize(e);
      const nn = normalize(n);
      const tt = normalize(t);
      return `| ${mm} | ${ee} | ${nn} | ${tt} | ${name}\n`;
    };
    if (typeof type === 'string' && type.toLowerCase()[0] === 'a') {
      ruleStats.sort(sortAlpha);
      udtStats.sort(sortAlpha);
      out += '    RULES/UDTS ALPHABETICALLY\n';
    } else if (typeof type === 'string' && type.toLowerCase()[0] === 'i') {
      ruleStats.sort(sortIndex);
      udtStats.sort(sortIndex);
      out += '    RULES/UDTS BY INDEX\n';
    } else {
      ruleStats.sort(sortHits);
      udtStats.sort(sortHits);
      out += '    RULES/UDTS BY HIT COUNT\n';
    }
    out += '|   MATCH |   EMPTY | NOMATCH |   TOTAL | NAME\n';
    for (let i = 0; i < ruleStats.length; i += 1) {
      let r = ruleStats[i];
      if (r.total) {
        out += displayRow(r.match, r.empty, r.nomatch, r.total, r.name);
      }
    }
    for (let i = 0; i < udtStats.length; i += 1) {
      let r = udtStats[i];
      if (r.total) {
        out += displayRow(r.match, r.empty, r.nomatch, r.total, r.name);
      }
    }
    return out;
  };
  const normalize = (n) => {
    if (n < 10) {
      return `      ${n}`;
    }
    if (n < 100) {
      return `     ${n}`;
    }
    if (n < 1000) {
      return `    ${n}`;
    }
    if (n < 10000) {
      return `   ${n}`;
    }
    if (n < 100000) {
      return `  ${n}`;
    }
    if (n < 1000000) {
      return ` ${n}`;
    }
    return `${n}`;
  };
  const sortAlpha = (lhs, rhs) => {
    if (lhs.lower < rhs.lower) {
      return -1;
    }
    if (lhs.lower > rhs.lower) {
      return 1;
    }
    return 0;
  };
  const sortHits = (lhs, rhs) => {
    if (lhs.total < rhs.total) {
      return 1;
    }
    if (lhs.total > rhs.total) {
      return -1;
    }
    return sortAlpha(lhs, rhs);
  };
  const sortIndex = (lhs, rhs) => {
    if (lhs.index < rhs.index) {
      return -1;
    }
    if (lhs.index > rhs.index) {
      return 1;
    }
    return 0;
  };
  const EmptyStat = function fnempty() {
    this.empty = 0;
    this.match = 0;
    this.nomatch = 0;
    this.total = 0;
  };
  /* Zero out all stats */
  const clear = () => {
    stats.length = 0;
    totals = new EmptyStat();
    stats[id.ALT] = new EmptyStat();
    stats[id.CAT] = new EmptyStat();
    stats[id.REP] = new EmptyStat();
    stats[id.RNM] = new EmptyStat();
    stats[id.TRG] = new EmptyStat();
    stats[id.TBS] = new EmptyStat();
    stats[id.TLS] = new EmptyStat();
    stats[id.UDT] = new EmptyStat();
    stats[id.AND] = new EmptyStat();
    stats[id.NOT] = new EmptyStat();
    ruleStats.length = 0;
    for (let i = 0; i < rules.length; i += 1) {
      ruleStats.push({
        empty: 0,
        match: 0,
        nomatch: 0,
        total: 0,
        name: rules[i].name,
        lower: rules[i].lower,
        index: rules[i].index,
      });
    }
    if (udts.length > 0) {
      udtStats.length = 0;
      for (let i = 0; i < udts.length; i += 1) {
        udtStats.push({
          empty: 0,
          match: 0,
          nomatch: 0,
          total: 0,
          name: udts[i].name,
          lower: udts[i].lower,
          index: udts[i].index,
        });
      }
    }
  };
  /* increment the designated operator hit count by one */
  const incStat = (stat, state) => {
    stat.total += 1;
    switch (state) {
      case id.EMPTY:
        stat.empty += 1;
        break;
      case id.MATCH:
        stat.match += 1;
        break;
      case id.NOMATCH:
        stat.nomatch += 1;
        break;
      default:
        throw new Error(`${thisFileName}collect(): incStat(): unrecognized state: ${state}`);
    }
  };
};

const utilities = {
  // utility functions
  stringToChars: (string) => [...string].map((cp) => cp.codePointAt(0)),
  charsToString: (chars, beg, len) => {
    let subChars = chars;
    while (1) {
      if (beg === undefined || beg < 0) {
        break;
      }
      if (len === undefined) {
        subChars = chars.slice(beg);
        break;
      }
      if (len <= 0) {
        // always an empty string
        return '';
      }
      subChars = chars.slice(beg, beg + len);
      break;
    }
    return String.fromCodePoint(...subChars);
  },
};

const identifiers = {
  // Identifies the operator type.
  // NB: These must match the values in apg-js 4.3.0, apg-lib/identifiers.
  /* the original ABNF operators */
  ALT: 1 /* alternation */,
  CAT: 2 /* concatenation */,
  REP: 3 /* repetition */,
  RNM: 4 /* rule name */,
  TRG: 5 /* terminal range */,
  TBS: 6 /* terminal binary string, case sensitive */,
  TLS: 7 /* terminal literal string, case insensitive */,
  /* the super set, SABNF operators */
  UDT: 11 /* user-defined terminal */,
  AND: 12 /* positive look ahead */,
  NOT: 13 /* negative look ahead */,
  // Used by the parser and the user's `RNM` and `UDT` callback functions.
  // Identifies the parser state as it traverses the parse tree nodes.
  // - *ACTIVE* - indicates the downward direction through the parse tree node.
  // - *MATCH* - indicates the upward direction and a phrase, of length \> 0, has been successfully matched
  // - *EMPTY* - indicates the upward direction and a phrase, of length = 0, has been successfully matched
  // - *NOMATCH* - indicates the upward direction and the parser failed to match any phrase at all
  ACTIVE: 100,
  MATCH: 101,
  EMPTY: 102,
  NOMATCH: 103,
  // Used by [`AST` translator](./ast.html) (semantic analysis) and the user's callback functions
  // to indicate the direction of flow through the `AST` nodes.
  // - *SEM_PRE* - indicates the downward (pre-branch) direction through the `AST` node.
  // - *SEM_POST* - indicates the upward (post-branch) direction through the `AST` node.
  SEM_PRE: 200,
  SEM_POST: 201,
  // Ignored. Retained for backwords compatibility.
  SEM_OK: 300,
  idName: (s) => {
    switch (s) {
      case identifiers.ALT:
        return 'ALT';
      case identifiers.CAT:
        return 'CAT';
      case identifiers.REP:
        return 'REP';
      case identifiers.RNM:
        return 'RNM';
      case identifiers.TRG:
        return 'TRG';
      case identifiers.TBS:
        return 'TBS';
      case identifiers.TLS:
        return 'TLS';
      case identifiers.UDT:
        return 'UDT';
      case identifiers.AND:
        return 'AND';
      case identifiers.NOT:
        return 'NOT';
      case identifiers.ACTIVE:
        return 'ACTIVE';
      case identifiers.EMPTY:
        return 'EMPTY';
      case identifiers.MATCH:
        return 'MATCH';
      case identifiers.NOMATCH:
        return 'NOMATCH';
      case identifiers.SEM_PRE:
        return 'SEM_PRE';
      case identifiers.SEM_POST:
        return 'SEM_POST';
      case identifiers.SEM_OK:
        return 'SEM_OK';
      default:
        return 'UNRECOGNIZED STATE';
    }
  },
};


/***/ }),

/***/ 678:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (/* binding */ grammar)
/* harmony export */ });
// copyright: Copyright (c) 2024 Lowell D. Thomas, all rights reserved<br>
//   license: BSD-2-Clause (https://opensource.org/licenses/BSD-2-Clause)<br>
//
// Generated by apg-js, Version 4.4.0 [apg-js](https://github.com/ldthomas/apg-js)
function grammar() {
  // ```
  // SUMMARY
  //      rules = 8
  //       udts = 0
  //    opcodes = 28
  //        ---   ABNF original opcodes
  //        ALT = 5
  //        CAT = 3
  //        REP = 3
  //        RNM = 6
  //        TLS = 5
  //        TBS = 1
  //        TRG = 5
  //        ---   SABNF superset opcodes
  //        UDT = 0
  //        AND = 0
  //        NOT = 0
  // characters = [0 - 1114111]
  // ```
  /* OBJECT IDENTIFIER (for internal parser use) */
  this.grammarObject = 'grammarObject';

  /* RULES */
  this.rules = [];
  this.rules[0] = {
    name: 'json-pointer',
    lower: 'json-pointer',
    index: 0,
    isBkr: false
  };
  this.rules[1] = {
    name: 'reference-token',
    lower: 'reference-token',
    index: 1,
    isBkr: false
  };
  this.rules[2] = {
    name: 'unescaped',
    lower: 'unescaped',
    index: 2,
    isBkr: false
  };
  this.rules[3] = {
    name: 'escaped',
    lower: 'escaped',
    index: 3,
    isBkr: false
  };
  this.rules[4] = {
    name: 'array-location',
    lower: 'array-location',
    index: 4,
    isBkr: false
  };
  this.rules[5] = {
    name: 'array-index',
    lower: 'array-index',
    index: 5,
    isBkr: false
  };
  this.rules[6] = {
    name: 'array-dash',
    lower: 'array-dash',
    index: 6,
    isBkr: false
  };
  this.rules[7] = {
    name: 'slash',
    lower: 'slash',
    index: 7,
    isBkr: false
  };

  /* UDTS */
  this.udts = [];

  /* OPCODES */
  /* json-pointer */
  this.rules[0].opcodes = [];
  this.rules[0].opcodes[0] = {
    type: 3,
    min: 0,
    max: Infinity
  }; // REP
  this.rules[0].opcodes[1] = {
    type: 2,
    children: [2, 3]
  }; // CAT
  this.rules[0].opcodes[2] = {
    type: 4,
    index: 7
  }; // RNM(slash)
  this.rules[0].opcodes[3] = {
    type: 4,
    index: 1
  }; // RNM(reference-token)

  /* reference-token */
  this.rules[1].opcodes = [];
  this.rules[1].opcodes[0] = {
    type: 3,
    min: 0,
    max: Infinity
  }; // REP
  this.rules[1].opcodes[1] = {
    type: 1,
    children: [2, 3]
  }; // ALT
  this.rules[1].opcodes[2] = {
    type: 4,
    index: 2
  }; // RNM(unescaped)
  this.rules[1].opcodes[3] = {
    type: 4,
    index: 3
  }; // RNM(escaped)

  /* unescaped */
  this.rules[2].opcodes = [];
  this.rules[2].opcodes[0] = {
    type: 1,
    children: [1, 2, 3]
  }; // ALT
  this.rules[2].opcodes[1] = {
    type: 5,
    min: 0,
    max: 46
  }; // TRG
  this.rules[2].opcodes[2] = {
    type: 5,
    min: 48,
    max: 125
  }; // TRG
  this.rules[2].opcodes[3] = {
    type: 5,
    min: 127,
    max: 1114111
  }; // TRG

  /* escaped */
  this.rules[3].opcodes = [];
  this.rules[3].opcodes[0] = {
    type: 2,
    children: [1, 2]
  }; // CAT
  this.rules[3].opcodes[1] = {
    type: 7,
    string: [126]
  }; // TLS
  this.rules[3].opcodes[2] = {
    type: 1,
    children: [3, 4]
  }; // ALT
  this.rules[3].opcodes[3] = {
    type: 7,
    string: [48]
  }; // TLS
  this.rules[3].opcodes[4] = {
    type: 7,
    string: [49]
  }; // TLS

  /* array-location */
  this.rules[4].opcodes = [];
  this.rules[4].opcodes[0] = {
    type: 1,
    children: [1, 2]
  }; // ALT
  this.rules[4].opcodes[1] = {
    type: 4,
    index: 5
  }; // RNM(array-index)
  this.rules[4].opcodes[2] = {
    type: 4,
    index: 6
  }; // RNM(array-dash)

  /* array-index */
  this.rules[5].opcodes = [];
  this.rules[5].opcodes[0] = {
    type: 1,
    children: [1, 2]
  }; // ALT
  this.rules[5].opcodes[1] = {
    type: 6,
    string: [48]
  }; // TBS
  this.rules[5].opcodes[2] = {
    type: 2,
    children: [3, 4]
  }; // CAT
  this.rules[5].opcodes[3] = {
    type: 5,
    min: 49,
    max: 57
  }; // TRG
  this.rules[5].opcodes[4] = {
    type: 3,
    min: 0,
    max: Infinity
  }; // REP
  this.rules[5].opcodes[5] = {
    type: 5,
    min: 48,
    max: 57
  }; // TRG

  /* array-dash */
  this.rules[6].opcodes = [];
  this.rules[6].opcodes[0] = {
    type: 7,
    string: [45]
  }; // TLS

  /* slash */
  this.rules[7].opcodes = [];
  this.rules[7].opcodes[0] = {
    type: 7,
    string: [47]
  }; // TLS

  // The `toString()` function will display the original grammar file(s) that produced these opcodes.
  this.toString = function toString() {
    let str = "";
    str += "; JavaScript Object Notation (JSON) Pointer ABNF syntax\n";
    str += "; https://datatracker.ietf.org/doc/html/rfc6901\n";
    str += "json-pointer    = *( slash reference-token ) ; MODIFICATION: surrogate text rule used\n";
    str += "reference-token = *( unescaped / escaped )\n";
    str += "unescaped       = %x00-2E / %x30-7D / %x7F-10FFFF\n";
    str += "                ; %x2F ('/') and %x7E ('~') are excluded from 'unescaped'\n";
    str += "escaped         = \"~\" ( \"0\" / \"1\" )\n";
    str += "                ; representing '~' and '/', respectively\n";
    str += "\n";
    str += "; https://datatracker.ietf.org/doc/html/rfc6901#section-4\n";
    str += "array-location  = array-index / array-dash\n";
    str += "array-index     = %x30 / ( %x31-39 *(%x30-39) )\n";
    str += "                ; \"0\", or digits without a leading \"0\"\n";
    str += "array-dash      = \"-\"\n";
    str += "\n";
    str += "; Surrogate named rules\n";
    str += "slash           = \"/\"\n";
    return str;
  };
}

/***/ }),

/***/ 682:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   from: () => (/* binding */ from),
/* harmony export */   fromURIReference: () => (/* binding */ fromURIReference),
/* harmony export */   to: () => (/* binding */ to)
/* harmony export */ });
const to = jsonPointer => {
  const encodedFragment = [...jsonPointer].map(char => /^[A-Za-z0-9\-._~!$&'()*+,;=:@/?]$/.test(char) ? char : encodeURIComponent(char)).join('');
  return `#${encodedFragment}`;
};
const from = fragment => {
  try {
    const rfc3986Fragment = fragment.startsWith('#') ? fragment.slice(1) : fragment;
    return decodeURIComponent(rfc3986Fragment);
  } catch {
    return fragment;
  }
};
const fromURIReference = uriReference => {
  const fragmentIndex = uriReference.indexOf('#');
  const fragment = fragmentIndex === -1 ? '#' : uriReference.substring(fragmentIndex);
  return from(fragment);
};

/***/ }),

/***/ 738:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
/* harmony import */ var apg_lite__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(646);
/* harmony import */ var _errors_JSONPointerParseError_js__WEBPACK_IMPORTED_MODULE_1__ = __webpack_require__(819);


const cst = ruleName => {
  return (state, chars, phraseIndex, phraseLength, data) => {
    if (!(typeof data === 'object' && data !== null && !Array.isArray(data))) {
      throw new _errors_JSONPointerParseError_js__WEBPACK_IMPORTED_MODULE_1__["default"]("parser's user data must be an object");
    }
    if (state === apg_lite__WEBPACK_IMPORTED_MODULE_0__.identifiers.SEM_PRE) {
      const node = {
        type: ruleName,
        text: apg_lite__WEBPACK_IMPORTED_MODULE_0__.utilities.charsToString(chars, phraseIndex, phraseLength),
        start: phraseIndex,
        length: phraseLength,
        children: []
      };
      if (data.stack.length > 0) {
        const parent = data.stack[data.stack.length - 1];
        parent.children.push(node);
      } else {
        data.root = node;
      }
      data.stack.push(node);
    }
    if (state === apg_lite__WEBPACK_IMPORTED_MODULE_0__.identifiers.SEM_POST) {
      data.stack.pop();
    }
  };
};
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (cst);

/***/ }),

/***/ 761:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   from: () => (/* binding */ from),
/* harmony export */   to: () => (/* binding */ to)
/* harmony export */ });
const to = jsonPointer => {
  return JSON.stringify(jsonPointer);
};
const from = jsonString => {
  try {
    return String(JSON.parse(jsonString));
  } catch {
    return jsonString;
  }
};

/***/ }),

/***/ 810:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
class Expectations extends Array {
  toString() {
    return this.map(c => `"${String(c)}"`).join(', ');
  }
}
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (Expectations);

/***/ }),

/***/ 819:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
/* harmony import */ var _JSONPointerError_js__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(628);

class JSONPointerParseError extends _JSONPointerError_js__WEBPACK_IMPORTED_MODULE_0__["default"] {}
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (JSONPointerParseError);

/***/ }),

/***/ 839:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
/* harmony import */ var _EvaluationRealm_js__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(133);
/* harmony import */ var _errors_JSONPointerIndexError_js__WEBPACK_IMPORTED_MODULE_1__ = __webpack_require__(874);


class JSONEvaluationRealm extends _EvaluationRealm_js__WEBPACK_IMPORTED_MODULE_0__["default"] {
  name = 'json';
  isArray(node) {
    return Array.isArray(node);
  }
  isObject(node) {
    return typeof node === 'object' && node !== null && !this.isArray(node);
  }
  sizeOf(node) {
    if (this.isArray(node)) {
      return node.length;
    }
    if (this.isObject(node)) {
      return Object.keys(node).length;
    }
    return 0;
  }
  has(node, referenceToken) {
    if (this.isArray(node)) {
      const index = Number(referenceToken);
      const indexUint32 = index >>> 0;
      if (index !== indexUint32) {
        throw new _errors_JSONPointerIndexError_js__WEBPACK_IMPORTED_MODULE_1__["default"](`Invalid array index "${referenceToken}": index must be an unsinged 32-bit integer`, {
          referenceToken,
          currentValue: node,
          realm: this.name
        });
      }
      return indexUint32 < this.sizeOf(node) && Object.prototype.hasOwnProperty.call(node, index);
    }
    if (this.isObject(node)) {
      return Object.prototype.hasOwnProperty.call(node, referenceToken);
    }
    return false;
  }
  evaluate(node, referenceToken) {
    if (this.isArray(node)) {
      return node[Number(referenceToken)];
    }
    return node[referenceToken];
  }
}
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (JSONEvaluationRealm);

/***/ }),

/***/ 853:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
/* harmony import */ var _JSONPointerError_js__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(628);

class JSONPointerCompileError extends _JSONPointerError_js__WEBPACK_IMPORTED_MODULE_0__["default"] {}
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (JSONPointerCompileError);

/***/ }),

/***/ 874:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
/* harmony import */ var _JSONPointerEvaluateError_js__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(369);

class JSONPointerIndexError extends _JSONPointerEvaluateError_js__WEBPACK_IMPORTED_MODULE_0__["default"] {}
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (JSONPointerIndexError);

/***/ }),

/***/ 922:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
/* harmony import */ var apg_lite__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(646);
/* harmony import */ var _grammar_js__WEBPACK_IMPORTED_MODULE_1__ = __webpack_require__(678);


const grammar = new _grammar_js__WEBPACK_IMPORTED_MODULE_1__["default"]();
const parser = new apg_lite__WEBPACK_IMPORTED_MODULE_0__.Parser();
const testArrayDash = referenceToken => {
  if (typeof referenceToken !== 'string') return false;
  try {
    return parser.parse(grammar, 'array-dash', referenceToken).success;
  } catch {
    return false;
  }
};
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (testArrayDash);

/***/ }),

/***/ 932:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
/* harmony import */ var _JSONPointerEvaluateError_js__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(369);

class JSONPointerTypeError extends _JSONPointerEvaluateError_js__WEBPACK_IMPORTED_MODULE_0__["default"] {}
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (JSONPointerTypeError);

/***/ }),

/***/ 979:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
/* harmony import */ var apg_lite__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(646);
/* harmony import */ var _callbacks_cst_js__WEBPACK_IMPORTED_MODULE_1__ = __webpack_require__(738);


class CSTTranslator extends apg_lite__WEBPACK_IMPORTED_MODULE_0__.Ast {
  constructor() {
    super();
    this.callbacks['json-pointer'] = (0,_callbacks_cst_js__WEBPACK_IMPORTED_MODULE_1__["default"])('json-pointer');
    this.callbacks['reference-token'] = (0,_callbacks_cst_js__WEBPACK_IMPORTED_MODULE_1__["default"])('reference-token');
    this.callbacks['slash'] = (0,_callbacks_cst_js__WEBPACK_IMPORTED_MODULE_1__["default"])('text');
  }
  getTree() {
    const data = {
      stack: [],
      root: null
    };
    this.translate(data);
    delete data.stack;
    return data;
  }
}
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (CSTTranslator);

/***/ }),

/***/ 980:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
const escape = referenceToken => {
  if (typeof referenceToken !== 'string' && typeof referenceToken !== 'number') {
    throw new TypeError('Reference token must be a string or number');
  }
  return String(referenceToken).replace(/~/g, '~0').replace(/\//g, '~1');
};
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (escape);

/***/ }),

/***/ 988:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
/* harmony import */ var _CSTTranslator_js__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(979);

class XMLTranslator extends _CSTTranslator_js__WEBPACK_IMPORTED_MODULE_0__["default"] {
  getTree() {
    return this.toXml();
  }
}
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (XMLTranslator);

/***/ }),

/***/ 992:
/***/ ((__unused_webpack___webpack_module__, __webpack_exports__, __webpack_require__) => {

__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   "default": () => (__WEBPACK_DEFAULT_EXPORT__)
/* harmony export */ });
/* harmony import */ var apg_lite__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(646);
/* harmony import */ var _Expectations_js__WEBPACK_IMPORTED_MODULE_1__ = __webpack_require__(810);


class Trace extends apg_lite__WEBPACK_IMPORTED_MODULE_0__.Trace {
  inferExpectations() {
    const lines = this.displayTrace().split('\n');
    const expectations = new Set();
    let collecting = false;
    let lastMatchedIndex = -1;
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];

      // capture the max match line (first one that ends in a single character match)
      if (!collecting && line.includes('M|')) {
        const textMatch = line.match(/]'(.*)'$/);
        if (textMatch && textMatch[1]) {
          lastMatchedIndex = i;
        }
      }

      // begin collecting after the deepest successful match
      if (i > lastMatchedIndex) {
        const terminalFailMatch = line.match(/N\|\[TLS\(([^)]+)\)]/);
        if (terminalFailMatch) {
          expectations.add(terminalFailMatch[1]);
        }
      }
    }
    return new _Expectations_js__WEBPACK_IMPORTED_MODULE_1__["default"](...expectations);
  }
}
/* harmony default export */ const __WEBPACK_DEFAULT_EXPORT__ = (Trace);

/***/ })

/******/ 	});
/************************************************************************/
/******/ 	// The module cache
/******/ 	var __webpack_module_cache__ = {};
/******/ 	
/******/ 	// The require function
/******/ 	function __webpack_require__(moduleId) {
/******/ 		// Check if module is in cache
/******/ 		var cachedModule = __webpack_module_cache__[moduleId];
/******/ 		if (cachedModule !== undefined) {
/******/ 			return cachedModule.exports;
/******/ 		}
/******/ 		// Create a new module (and put it into the cache)
/******/ 		var module = __webpack_module_cache__[moduleId] = {
/******/ 			// no module.id needed
/******/ 			// no module.loaded needed
/******/ 			exports: {}
/******/ 		};
/******/ 	
/******/ 		// Execute the module function
/******/ 		__webpack_modules__[moduleId](module, module.exports, __webpack_require__);
/******/ 	
/******/ 		// Return the exports of the module
/******/ 		return module.exports;
/******/ 	}
/******/ 	
/************************************************************************/
/******/ 	/* webpack/runtime/define property getters */
/******/ 	(() => {
/******/ 		// define getter functions for harmony exports
/******/ 		__webpack_require__.d = (exports, definition) => {
/******/ 			for(var key in definition) {
/******/ 				if(__webpack_require__.o(definition, key) && !__webpack_require__.o(exports, key)) {
/******/ 					Object.defineProperty(exports, key, { enumerable: true, get: definition[key] });
/******/ 				}
/******/ 			}
/******/ 		};
/******/ 	})();
/******/ 	
/******/ 	/* webpack/runtime/hasOwnProperty shorthand */
/******/ 	(() => {
/******/ 		__webpack_require__.o = (obj, prop) => (Object.prototype.hasOwnProperty.call(obj, prop))
/******/ 	})();
/******/ 	
/******/ 	/* webpack/runtime/make namespace object */
/******/ 	(() => {
/******/ 		// define __esModule on exports
/******/ 		__webpack_require__.r = (exports) => {
/******/ 			if(typeof Symbol !== 'undefined' && Symbol.toStringTag) {
/******/ 				Object.defineProperty(exports, Symbol.toStringTag, { value: 'Module' });
/******/ 			}
/******/ 			Object.defineProperty(exports, '__esModule', { value: true });
/******/ 		};
/******/ 	})();
/******/ 	
/************************************************************************/
var __webpack_exports__ = {};
__webpack_require__.r(__webpack_exports__);
/* harmony export */ __webpack_require__.d(__webpack_exports__, {
/* harmony export */   ASTTranslator: () => (/* reexport safe */ _parse_translators_ASTTranslator_js__WEBPACK_IMPORTED_MODULE_5__["default"]),
/* harmony export */   CSTTranslator: () => (/* reexport safe */ _parse_translators_CSTTranslator_js__WEBPACK_IMPORTED_MODULE_4__["default"]),
/* harmony export */   EvaluationRealm: () => (/* reexport safe */ _evaluate_realms_EvaluationRealm_js__WEBPACK_IMPORTED_MODULE_16__["default"]),
/* harmony export */   Grammar: () => (/* reexport safe */ _grammar_js__WEBPACK_IMPORTED_MODULE_2__["default"]),
/* harmony export */   JSONPointerCompileError: () => (/* reexport safe */ _errors_JSONPointerCompileError_js__WEBPACK_IMPORTED_MODULE_20__["default"]),
/* harmony export */   JSONPointerError: () => (/* reexport safe */ _errors_JSONPointerError_js__WEBPACK_IMPORTED_MODULE_18__["default"]),
/* harmony export */   JSONPointerEvaluateError: () => (/* reexport safe */ _errors_JSONPointerEvaluateError_js__WEBPACK_IMPORTED_MODULE_21__["default"]),
/* harmony export */   JSONPointerIndexError: () => (/* reexport safe */ _errors_JSONPointerIndexError_js__WEBPACK_IMPORTED_MODULE_24__["default"]),
/* harmony export */   JSONPointerKeyError: () => (/* reexport safe */ _errors_JSONPointerKeyError_js__WEBPACK_IMPORTED_MODULE_23__["default"]),
/* harmony export */   JSONPointerParseError: () => (/* reexport safe */ _errors_JSONPointerParseError_js__WEBPACK_IMPORTED_MODULE_19__["default"]),
/* harmony export */   JSONPointerTypeError: () => (/* reexport safe */ _errors_JSONPointerTypeError_js__WEBPACK_IMPORTED_MODULE_22__["default"]),
/* harmony export */   JSONString: () => (/* reexport module object */ _representation_json_string_js__WEBPACK_IMPORTED_MODULE_0__),
/* harmony export */   URIFragmentIdentifier: () => (/* reexport module object */ _representation_uri_fragment_identifier_js__WEBPACK_IMPORTED_MODULE_1__),
/* harmony export */   XMLTranslator: () => (/* reexport safe */ _parse_translators_XMLTranslator_js__WEBPACK_IMPORTED_MODULE_6__["default"]),
/* harmony export */   compile: () => (/* reexport safe */ _compile_js__WEBPACK_IMPORTED_MODULE_12__["default"]),
/* harmony export */   composeRealms: () => (/* reexport safe */ _evaluate_realms_compose_js__WEBPACK_IMPORTED_MODULE_17__["default"]),
/* harmony export */   escape: () => (/* reexport safe */ _escape_js__WEBPACK_IMPORTED_MODULE_13__["default"]),
/* harmony export */   evaluate: () => (/* reexport safe */ _evaluate_index_js__WEBPACK_IMPORTED_MODULE_15__["default"]),
/* harmony export */   parse: () => (/* reexport safe */ _parse_index_js__WEBPACK_IMPORTED_MODULE_3__["default"]),
/* harmony export */   testArrayDash: () => (/* reexport safe */ _test_array_dash_js__WEBPACK_IMPORTED_MODULE_11__["default"]),
/* harmony export */   testArrayIndex: () => (/* reexport safe */ _test_array_index_js__WEBPACK_IMPORTED_MODULE_10__["default"]),
/* harmony export */   testArrayLocation: () => (/* reexport safe */ _test_array_location_js__WEBPACK_IMPORTED_MODULE_9__["default"]),
/* harmony export */   testJSONPointer: () => (/* reexport safe */ _test_json_pointer_js__WEBPACK_IMPORTED_MODULE_7__["default"]),
/* harmony export */   testReferenceToken: () => (/* reexport safe */ _test_reference_token_js__WEBPACK_IMPORTED_MODULE_8__["default"]),
/* harmony export */   unescape: () => (/* reexport safe */ _unescape_js__WEBPACK_IMPORTED_MODULE_14__["default"])
/* harmony export */ });
/* harmony import */ var _representation_json_string_js__WEBPACK_IMPORTED_MODULE_0__ = __webpack_require__(761);
/* harmony import */ var _representation_uri_fragment_identifier_js__WEBPACK_IMPORTED_MODULE_1__ = __webpack_require__(682);
/* harmony import */ var _grammar_js__WEBPACK_IMPORTED_MODULE_2__ = __webpack_require__(678);
/* harmony import */ var _parse_index_js__WEBPACK_IMPORTED_MODULE_3__ = __webpack_require__(583);
/* harmony import */ var _parse_translators_CSTTranslator_js__WEBPACK_IMPORTED_MODULE_4__ = __webpack_require__(979);
/* harmony import */ var _parse_translators_ASTTranslator_js__WEBPACK_IMPORTED_MODULE_5__ = __webpack_require__(142);
/* harmony import */ var _parse_translators_XMLTranslator_js__WEBPACK_IMPORTED_MODULE_6__ = __webpack_require__(988);
/* harmony import */ var _test_json_pointer_js__WEBPACK_IMPORTED_MODULE_7__ = __webpack_require__(544);
/* harmony import */ var _test_reference_token_js__WEBPACK_IMPORTED_MODULE_8__ = __webpack_require__(533);
/* harmony import */ var _test_array_location_js__WEBPACK_IMPORTED_MODULE_9__ = __webpack_require__(525);
/* harmony import */ var _test_array_index_js__WEBPACK_IMPORTED_MODULE_10__ = __webpack_require__(348);
/* harmony import */ var _test_array_dash_js__WEBPACK_IMPORTED_MODULE_11__ = __webpack_require__(922);
/* harmony import */ var _compile_js__WEBPACK_IMPORTED_MODULE_12__ = __webpack_require__(58);
/* harmony import */ var _escape_js__WEBPACK_IMPORTED_MODULE_13__ = __webpack_require__(980);
/* harmony import */ var _unescape_js__WEBPACK_IMPORTED_MODULE_14__ = __webpack_require__(415);
/* harmony import */ var _evaluate_index_js__WEBPACK_IMPORTED_MODULE_15__ = __webpack_require__(427);
/* harmony import */ var _evaluate_realms_EvaluationRealm_js__WEBPACK_IMPORTED_MODULE_16__ = __webpack_require__(133);
/* harmony import */ var _evaluate_realms_compose_js__WEBPACK_IMPORTED_MODULE_17__ = __webpack_require__(632);
/* harmony import */ var _errors_JSONPointerError_js__WEBPACK_IMPORTED_MODULE_18__ = __webpack_require__(628);
/* harmony import */ var _errors_JSONPointerParseError_js__WEBPACK_IMPORTED_MODULE_19__ = __webpack_require__(819);
/* harmony import */ var _errors_JSONPointerCompileError_js__WEBPACK_IMPORTED_MODULE_20__ = __webpack_require__(853);
/* harmony import */ var _errors_JSONPointerEvaluateError_js__WEBPACK_IMPORTED_MODULE_21__ = __webpack_require__(369);
/* harmony import */ var _errors_JSONPointerTypeError_js__WEBPACK_IMPORTED_MODULE_22__ = __webpack_require__(932);
/* harmony import */ var _errors_JSONPointerKeyError_js__WEBPACK_IMPORTED_MODULE_23__ = __webpack_require__(71);
/* harmony import */ var _errors_JSONPointerIndexError_js__WEBPACK_IMPORTED_MODULE_24__ = __webpack_require__(874);


























/******/ 	return __webpack_exports__;
/******/ })()
;
});