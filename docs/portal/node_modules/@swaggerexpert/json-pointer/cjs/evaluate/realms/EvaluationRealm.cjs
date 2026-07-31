"use strict";

exports.__esModule = true;
exports.default = void 0;
var _JSONPointerError = _interopRequireDefault(require("../../errors/JSONPointerError.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
class EvaluationRealm {
  name = '';
  isArray(node) {
    throw new _JSONPointerError.default('Realm.isArray(node) must be implemented in a subclass');
  }
  isObject(node) {
    throw new _JSONPointerError.default('Realm.isObject(node) must be implemented in a subclass');
  }
  sizeOf(node) {
    throw new _JSONPointerError.default('Realm.sizeOf(node) must be implemented in a subclass');
  }
  has(node, referenceToken) {
    throw new _JSONPointerError.default('Realm.has(node) must be implemented in a subclass');
  }
  evaluate(node, referenceToken) {
    throw new _JSONPointerError.default('Realm.evaluate(node) must be implemented in a subclass');
  }
}
var _default = exports.default = EvaluationRealm;