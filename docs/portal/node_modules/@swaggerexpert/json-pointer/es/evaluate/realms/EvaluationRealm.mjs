import JSONPointerError from "../../errors/JSONPointerError.mjs";
class EvaluationRealm {
  name = '';
  isArray(node) {
    throw new JSONPointerError('Realm.isArray(node) must be implemented in a subclass');
  }
  isObject(node) {
    throw new JSONPointerError('Realm.isObject(node) must be implemented in a subclass');
  }
  sizeOf(node) {
    throw new JSONPointerError('Realm.sizeOf(node) must be implemented in a subclass');
  }
  has(node, referenceToken) {
    throw new JSONPointerError('Realm.has(node) must be implemented in a subclass');
  }
  evaluate(node, referenceToken) {
    throw new JSONPointerError('Realm.evaluate(node) must be implemented in a subclass');
  }
}
export default EvaluationRealm;