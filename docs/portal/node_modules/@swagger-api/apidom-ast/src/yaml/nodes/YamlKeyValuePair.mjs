import Node from "../../Node.mjs";
import { isScalar, isMapping, isSequence, isAlias } from "./predicates.mjs";
/**
 * @public
 */
/**
 * @public
 */
class YamlKeyValuePair extends Node {
  static type = 'keyValuePair';
  styleGroup;
  constructor({
    styleGroup,
    ...rest
  }) {
    super({
      ...rest
    });
    this.styleGroup = styleGroup;
  }
}
Object.defineProperties(YamlKeyValuePair.prototype, {
  key: {
    get() {
      return this.children.filter(node => isScalar(node) || isMapping(node) || isSequence(node))[0];
    },
    enumerable: true
  },
  value: {
    get() {
      const {
        key,
        children
      } = this;
      const excludeKeyPredicate = node => node !== key;
      const valuePredicate = node => isScalar(node) || isMapping(node) || isSequence(node) || isAlias(node);
      return children.filter(node => excludeKeyPredicate(node) && valuePredicate(node))[0];
    },
    enumerable: true
  }
});
export default YamlKeyValuePair;