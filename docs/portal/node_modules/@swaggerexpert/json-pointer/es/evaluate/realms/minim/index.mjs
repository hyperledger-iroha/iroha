import { ObjectElement, ArrayElement } from 'minim';
import EvaluationRealm from "../EvaluationRealm.mjs";
import JSONPointerKeyError from "../../../errors/JSONPointerKeyError.mjs";
import JSONPointerIndexError from "../../../errors/JSONPointerIndexError.mjs";
class MinimEvaluationRealm extends EvaluationRealm {
  name = 'minim';
  isArray(node) {
    return node instanceof ArrayElement && !(node instanceof ObjectElement);
  }
  isObject(node) {
    return node instanceof ObjectElement;
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
        throw new JSONPointerIndexError(`Invalid array index "${referenceToken}": index must be an unsinged 32-bit integer`, {
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
        throw new JSONPointerKeyError(`Object key "${referenceToken}" is not unique — JSON Pointer requires unique member names`, {
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
export default MinimEvaluationRealm;