import YamlCollection from "./YamlCollection.mjs";
import { isMapping, isScalar, isSequence, isAlias } from "./predicates.mjs";
/**
 * @public
 */
class YamlSequence extends YamlCollection {
  static type = 'sequence';
}
Object.defineProperty(YamlSequence.prototype, 'content', {
  get() {
    const {
      children
    } = this;
    return Array.isArray(children) ? children.filter(node => isSequence(node) || isMapping(node) || isScalar(node) || isAlias(node)) : [];
  },
  enumerable: true
});
export default YamlSequence;