import EvaluationRealm from "../EvaluationRealm.mjs";
import JSONPointerIndexError from "../../../errors/JSONPointerIndexError.mjs";
class JSONEvaluationRealm extends EvaluationRealm {
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
        throw new JSONPointerIndexError(`Invalid array index "${referenceToken}": index must be an unsinged 32-bit integer`, {
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
export default JSONEvaluationRealm;