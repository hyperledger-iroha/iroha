"use strict";

exports.__esModule = true;
exports.default = void 0;
var _EvaluationRealm = _interopRequireDefault(require("./EvaluationRealm.cjs"));
var _JSONPointerEvaluateError = _interopRequireDefault(require("../../errors/JSONPointerEvaluateError.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
function _classPrivateFieldLooseBase(e, t) { if (!{}.hasOwnProperty.call(e, t)) throw new TypeError("attempted to use private field on non-instance"); return e; }
var id = 0;
function _classPrivateFieldLooseKey(e) { return "__private_" + id++ + "_" + e; }
var _findRealm = /*#__PURE__*/_classPrivateFieldLooseKey("findRealm");
class CompositeEvaluationRealm extends _EvaluationRealm.default {
  constructor(realms) {
    super();
    Object.defineProperty(this, _findRealm, {
      value: _findRealm2
    });
    this.name = 'composite';
    this.realms = [];
    this.realms = realms;
  }
  isArray(node) {
    return _classPrivateFieldLooseBase(this, _findRealm)[_findRealm](node).isArray(node);
  }
  isObject(node) {
    return _classPrivateFieldLooseBase(this, _findRealm)[_findRealm](node).isObject(node);
  }
  sizeOf(node) {
    return _classPrivateFieldLooseBase(this, _findRealm)[_findRealm](node).sizeOf(node);
  }
  has(node, referenceToken) {
    return _classPrivateFieldLooseBase(this, _findRealm)[_findRealm](node).has(node, referenceToken);
  }
  evaluate(node, referenceToken) {
    return _classPrivateFieldLooseBase(this, _findRealm)[_findRealm](node).evaluate(node, referenceToken);
  }
}
function _findRealm2(node) {
  for (const realm of this.realms) {
    if (realm.isArray(node) || realm.isObject(node)) {
      return realm;
    }
  }
  throw new _JSONPointerEvaluateError.default('No suitable evaluation realm found for value', {
    currentValue: node
  });
}
const compose = (...realms) => new CompositeEvaluationRealm(realms);
var _default = exports.default = compose;