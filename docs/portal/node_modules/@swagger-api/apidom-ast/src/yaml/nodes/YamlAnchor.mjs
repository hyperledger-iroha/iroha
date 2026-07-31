import Node from "../../Node.mjs";
/**
 * @public
 */
/**
 * @public
 */
class YamlAnchor extends Node {
  static type = 'anchor';
  name;
  constructor({
    name,
    ...rest
  }) {
    super({
      ...rest
    });
    this.name = name;
  }
}
export default YamlAnchor;