import YamlCollection from "./YamlCollection.mjs";
import { isKeyValuePair } from "./predicates.mjs";
/**
 * @public
 */
class YamlMapping extends YamlCollection {
  static type = 'mapping';
}
Object.defineProperty(YamlMapping.prototype, 'content', {
  get() {
    return Array.isArray(this.children) ? this.children.filter(isKeyValuePair) : [];
  },
  enumerable: true
});
export default YamlMapping;