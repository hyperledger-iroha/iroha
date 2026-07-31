import EvaluationRealm from "../EvaluationRealm.mjs";
import JSONPointerIndexError from "../../../errors/JSONPointerIndexError.mjs";
class MapSetEvaluationRealm extends EvaluationRealm {
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
          throw new JSONPointerIndexError(`Invalid array index "${referenceToken}": out of bounds`, {
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
export default MapSetEvaluationRealm;