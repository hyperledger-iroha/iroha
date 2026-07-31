import Node from "../../Node.mjs";
/**
 * @public
 */
/**
 * @public
 */
class YamlNode extends Node {
  anchor;
  tag;
  style;
  styleGroup;
  constructor({
    anchor,
    tag,
    style,
    styleGroup,
    ...rest
  }) {
    super({
      ...rest
    });
    this.anchor = anchor;
    this.tag = tag;
    this.style = style;
    this.styleGroup = styleGroup;
  }
}
export default YamlNode;