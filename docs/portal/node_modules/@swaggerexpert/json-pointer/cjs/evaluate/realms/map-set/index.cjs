"use strict";

exports.__esModule = true;
exports.default = void 0;
var _EvaluationRealm = _interopRequireDefault(require("../EvaluationRealm.cjs"));
var _JSONPointerIndexError = _interopRequireDefault(require("../../../errors/JSONPointerIndexError.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
class MapSetEvaluationRealm extends _EvaluationRealm.default {
  name = 'map-set';
  isArray(node) {
    return node instanceof Set || Object.prototype.toString.call(node) === '[object Set]';
  }
  isObject(node) {
    return node instanceof Map || Object.prototype.toString.call(node) === '[object Map]';
  }
  sizeOf(node) {
    if (this.isArray(node) || this.isObject(node)) {
      return node.size;
    }
    return 0;
  }
  has(node, referenceToken) {
    if (this.isArray(node)) {
      return Number(referenceToken) < this.sizeOf(node);
    }
    if (this.isObject(node)) {
      return node.has(referenceToken);
    }
    return false;
  }
  evaluate(node, referenceToken) {
    if (this.isArray(node)) {
      const index = Number(referenceToken);
      const iterator = node.values();
      for (let i = 0; i < index; i += 1) {
        if (iterator.next().done) {
          throw new _JSONPointerIndexError.default(`Invalid array index "${referenceToken}": out of bounds`, {
            referenceToken,
            currentValue: node,
            realm: this.name
          });
        }
      }
      return iterator.next().value;
    }
    return node.get(referenceToken);
  }
}
var _default = exports.default = MapSetEvaluationRealm;