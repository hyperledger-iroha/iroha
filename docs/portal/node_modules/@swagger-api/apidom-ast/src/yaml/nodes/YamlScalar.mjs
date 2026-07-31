import YamlNode from "./YamlNode.mjs";
/**
 * @public
 */
/**
 * @public
 */
class YamlScalar extends YamlNode {
  static type = 'scalar';
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
export default YamlScalar;