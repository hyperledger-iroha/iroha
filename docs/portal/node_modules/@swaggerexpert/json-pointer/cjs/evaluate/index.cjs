"use strict";

exports.__esModule = true;
exports.default = void 0;
var _index = _interopRequireDefault(require("../parse/index.cjs"));
var _arrayDash = _interopRequireDefault(require("../test/array-dash.cjs"));
var _arrayIndex = _interopRequireDefault(require("../test/array-index.cjs"));
var _TraceBuilder = _interopRequireDefault(require("./trace/TraceBuilder.cjs"));
var _index2 = _interopRequireDefault(require("./realms/json/index.cjs"));
var _JSONPointerEvaluateError = _interopRequireDefault(require("../errors/JSONPointerEvaluateError.cjs"));
var _JSONPointerTypeError = _interopRequireDefault(require("../errors/JSONPointerTypeError.cjs"));
var _JSONPointerIndexError = _interopRequireDefault(require("../errors/JSONPointerIndexError.cjs"));
var _JSONPointerKeyError = _interopRequireDefault(require("../errors/JSONPointerKeyError.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const evaluate = (value, jsonPointer, {
  strictArrays = true,
  strictObjects = true,
  realm = new _index2.default(),
  trace = true
} = {}) => {
  const {
    result: parseResult,
    tree: referenceTokens,
    trace: parseTrace
  } = (0, _index.default)(jsonPointer, {
    trace: !!trace
  });
  const tracer = typeof trace === 'object' && trace !== null ? new _TraceBuilder.default(trace, {
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
      throw new _JSONPointerEvaluateError.default(message, {
        jsonPointer,
        currentValue: value,
        realm: realm.name
      });
    }
    return referenceTokens.reduce((current, referenceToken, referenceTokenPosition) => {
      if (realm.isArray(current)) {
        if ((0, _arrayDash.default)(referenceToken)) {
          if (strictArrays) {
            throw new _JSONPointerIndexError.default(`Invalid array index "-" at position ${referenceTokenPosition} in "${jsonPointer}". The "-" token always refers to a nonexistent element during evaluation`, {
              jsonPointer,
              referenceTokens,
              referenceToken,
              referenceTokenPosition,
              currentValue: current,
              realm: realm.name
            });
          } else {
            output = realm.evaluate(current, String(realm.sizeOf(current)));
            tracer == null || tracer.step({
              referenceToken,
              input: current,
              output
            });
            return output;
          }
        }
        if (!(0, _arrayIndex.default)(referenceToken)) {
          throw new _JSONPointerIndexError.default(`Invalid array index "${referenceToken}" at position ${referenceTokenPosition} in "${jsonPointer}": index MUST be "0", or digits without a leading "0"`, {
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
          throw new _JSONPointerIndexError.default(`Invalid array index "${referenceToken}" at position ${referenceTokenPosition} in "${jsonPointer}": index must be a safe integer`, {
            jsonPointer,
            referenceTokens,
            referenceToken,
            referenceTokenPosition,
            currentValue: current,
            realm: realm.name
          });
        }
        if (!realm.has(current, referenceToken) && strictArrays) {
          throw new _JSONPointerIndexError.default(`Invalid array index "${referenceToken}" at position ${referenceTokenPosition} in "${jsonPointer}": index not found in array`, {
            jsonPointer,
            referenceTokens,
            referenceToken,
            referenceTokenPosition,
            currentValue: current,
            realm: realm.name
          });
        }
        output = realm.evaluate(current, referenceToken);
        tracer == null || tracer.step({
          referenceToken,
          input: current,
          output
        });
        return output;
      }
      if (realm.isObject(current)) {
        if (!realm.has(current, referenceToken) && strictObjects) {
          throw new _JSONPointerKeyError.default(`Invalid object key "${referenceToken}" at position ${referenceTokenPosition} in "${jsonPointer}": key not found in object`, {
            jsonPointer,
            referenceTokens,
            referenceToken,
            referenceTokenPosition,
            currentValue: current,
            realm: realm.name
          });
        }
        output = realm.evaluate(current, referenceToken);
        tracer == null || tracer.step({
          referenceToken,
          input: current,
          output
        });
        return output;
      }
      throw new _JSONPointerTypeError.default(`Invalid reference token "${referenceToken}" at position ${referenceTokenPosition} in "${jsonPointer}": cannot be applied to a non-object/non-array value`, {
        jsonPointer,
        referenceTokens,
        referenceToken,
        referenceTokenPosition,
        currentValue: current,
        realm: realm.name
      });
    }, value);
  } catch (error) {
    tracer == null || tracer.step({
      referenceToken: error.referenceToken,
      input: error.currentValue,
      success: false,
      reason: error.message
    });
    if (error instanceof _JSONPointerEvaluateError.default) {
      throw error;
    }
    throw new _JSONPointerEvaluateError.default('Unexpected error during JSON Pointer evaluation', {
      cause: error,
      jsonPointer,
      referenceTokens
    });
  }
};
var _default = exports.default = evaluate;