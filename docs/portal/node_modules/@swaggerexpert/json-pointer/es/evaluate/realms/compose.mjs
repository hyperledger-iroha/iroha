function _classPrivateMethodInitSpec(e, a) { _checkPrivateRedeclaration(e, a), a.add(e); }
function _checkPrivateRedeclaration(e, t) { if (t.has(e)) throw new TypeError("Cannot initialize the same private elements twice on an object"); }
function _defineProperty(e, r, t) { return (r = _toPropertyKey(r)) in e ? Object.defineProperty(e, r, { value: t, enumerable: !0, configurable: !0, writable: !0 }) : e[r] = t, e; }
function _toPropertyKey(t) { var i = _toPrimitive(t, "string"); return "symbol" == typeof i ? i : i + ""; }
function _toPrimitive(t, r) { if ("object" != typeof t || !t) return t; var e = t[Symbol.toPrimitive]; if (void 0 !== e) { var i = e.call(t, r || "default"); if ("object" != typeof i) return i; throw new TypeError("@@toPrimitive must return a primitive value."); } return ("string" === r ? String : Number)(t); }
function _assertClassBrand(e, t, n) { if ("function" == typeof e ? e === t : e.has(t)) return arguments.length < 3 ? t : n; throw new TypeError("Private element is not present on this object"); }
import EvaluationRealm from "./EvaluationRealm.mjs";
import JSONPointerEvaluateError from "../../errors/JSONPointerEvaluateError.mjs";
var _CompositeEvaluationRealm_brand = /*#__PURE__*/new WeakSet();
class CompositeEvaluationRealm extends EvaluationRealm {
  constructor(realms) {
    super();
    _classPrivateMethodInitSpec(this, _CompositeEvaluationRealm_brand);
    _defineProperty(this, "name", 'composite');
    _defineProperty(this, "realms", []);
    this.realms = realms;
  }
  isArray(node) {
    return _assertClassBrand(_CompositeEvaluationRealm_brand, this, _findRealm).call(this, node).isArray(node);
  }
  isObject(node) {
    return _assertClassBrand(_CompositeEvaluationRealm_brand, this, _findRealm).call(this, node).isObject(node);
  }
  sizeOf(node) {
    return _assertClassBrand(_CompositeEvaluationRealm_brand, this, _findRealm).call(this, node).sizeOf(node);
  }
  has(node, referenceToken) {
    return _assertClassBrand(_CompositeEvaluationRealm_brand, this, _findRealm).call(this, node).has(node, referenceToken);
  }
  evaluate(node, referenceToken) {
    return _assertClassBrand(_CompositeEvaluationRealm_brand, this, _findRealm).call(this, node).evaluate(node, referenceToken);
  }
}
function _findRealm(node) {
  for (const realm of this.realms) {
    if (realm.isArray(node) || realm.isObject(node)) {
      return realm;
    }
  }
  throw new JSONPointerEvaluateError('No suitable evaluation realm found for value', {
    currentValue: node
  });
}
const compose = (...realms) => new CompositeEvaluationRealm(realms);
export default compose;