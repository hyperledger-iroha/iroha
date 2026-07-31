import JsonNode from "./JsonNode.mjs";
/**
 * @public
 */
/**
 * @public
 */
class JsonValue extends JsonNode {
  static type = 'value';
  value;
  constructor({
    value,
    ...rest
  }) {
    super({
      ...rest
    });
    this.value = value;
  }
}
export default JsonValue;