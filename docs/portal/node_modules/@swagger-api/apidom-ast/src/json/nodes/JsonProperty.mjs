import JsonNode from "./JsonNode.mjs";
import { isArray, isFalse, isKey, isNull, isNumber, isObject, isString, isTrue } from "./predicates.mjs";
/**
 * @public
 */
/**
 * @public
 */
class JsonProperty extends JsonNode {
  static type = 'property';
  get key() {
    return this.children.find(isKey);
  }
  get value() {
    return this.children.find(node => isFalse(node) || isTrue(node) || isNull(node) || isNumber(node) || isString(node) || isArray(node) || isObject(node));
  }
}
export default JsonProperty;