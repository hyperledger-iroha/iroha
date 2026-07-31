import JsonNode from "./JsonNode.mjs";
import { isEscapeSequence, isStringContent } from "./predicates.mjs";
/**
 * @public
 */
class JsonString extends JsonNode {
  static type = 'string';
  get value() {
    if (this.children.length === 1) {
      const onlyChild = this.children[0];
      return onlyChild.value;
    }
    return this.children.filter(node => isStringContent(node) || isEscapeSequence(node)).reduce((acc, cur) => acc + cur.value, '');
  }
}
export default JsonString;