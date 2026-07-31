import Node from "../../Node.mjs";
/**
 * @public
 */
/**
 * @public
 */
class YamlComment extends Node {
  static type = 'comment';
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
export default YamlComment;