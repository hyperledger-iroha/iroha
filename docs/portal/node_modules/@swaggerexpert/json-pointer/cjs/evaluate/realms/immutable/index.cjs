"use strict";

exports.__esModule = true;
exports.default = void 0;
var _immutable = _interopRequireDefault(require("immutable"));
var _EvaluationRealm = _interopRequireDefault(require("../EvaluationRealm.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const {
  List,
  Map
} = _immutable.default;
class ImmutableEvaluationRealm extends _EvaluationRealm.default {
  name = 'immutable';
  isArray(node) {
    return List.isList(node);
  }
  isObject(node) {
    return Map.isMap(node);
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
      return node.get(Number(referenceToken));
    }
    return node.get(referenceToken);
  }
}
var _default = exports.default = ImmutableEvaluationRealm;