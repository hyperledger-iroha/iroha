"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _EvaluationRealm = _interopRequireDefault(require("../EvaluationRealm.cjs"));
var _JSONPointerKeyError = _interopRequireDefault(require("../../../errors/JSONPointerKeyError.cjs"));
var _JSONPointerIndexError = _interopRequireDefault(require("../../../errors/JSONPointerIndexError.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
class ApiDOMEvaluationRealm extends _EvaluationRealm.default {
  name = 'apidom';
  isArray(node) {
    return (0, _apidomCore.isArrayElement)(node);
  }
  isObject(node) {
    return (0, _apidomCore.isObjectElement)(node);
  }
  sizeOf(node) {
    if (this.isArray(node) || this.isObject(node)) {
      return node.length;
    }
    return 0;
  }
  has(node, referenceToken) {
    if (this.isArray(node)) {
      const index = Number(referenceToken);
      const indexUint32 = index >>> 0;
      if (index !== indexUint32) {
        throw new _JSONPointerIndexError.default(`Invalid array index "${referenceToken}": index must be an unsinged 32-bit integer`, {
          referenceToken,
          currentValue: node,
          realm: this.name
        });
      }
      return indexUint32 < this.sizeOf(node);
    }
    if (this.isObject(node)) {
      const keys = node.keys();
      const uniqueKeys = new Set(keys);
      if (keys.length !== uniqueKeys.size) {
        throw new _JSONPointerKeyError.default(`Object key "${referenceToken}" is not unique — JSON Pointer requires unique member names`, {
          referenceToken,
          currentValue: node,
          realm: this.name
        });
      }
      return node.hasKey(referenceToken);
    }
    return false;
  }
  evaluate(node, referenceToken) {
    if (this.isArray(node)) {
      return node.get(Number(referenceToken));
    }
    return node.get(referenceToken);
  }
}
var _default = exports.default = ApiDOMEvaluationRealm;