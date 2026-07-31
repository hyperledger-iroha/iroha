import Immutable from 'immutable';
import EvaluationRealm from "../EvaluationRealm.mjs";
const {
  List,
  Map
} = Immutable;
class ImmutableEvaluationRealm extends EvaluationRealm {
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
export default ImmutableEvaluationRealm;