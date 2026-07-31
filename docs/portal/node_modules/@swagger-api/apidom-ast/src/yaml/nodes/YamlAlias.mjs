import Node from "../../Node.mjs";
/**
 * @public
 */
/**
 * @public
 */
class YamlAlias extends Node {
  static type = 'alias';
  content;
  constructor({
    content,
    ...rest
  }) {
    super({
      ...rest
    });
    this.content = content;
  }
}
export default YamlAlias;