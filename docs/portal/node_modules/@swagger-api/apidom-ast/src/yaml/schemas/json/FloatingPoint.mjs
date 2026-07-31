import Tag from "../Tag.mjs";
/* eslint-disable class-methods-use-this */
class FloatingPoint extends Tag {
  static uri = 'tag:yaml.org,2002:float';
  test(node) {
    return /^-?(0|[1-9][0-9]*)(\.[0-9]*)?([eE][-+]?[0-9]+)?$/.test(node.content);
  }
  resolve(node) {
    const content = parseFloat(node.content);
    const nodeClone = node.clone();
    nodeClone.content = content;
    return nodeClone;
  }
}
/* eslint-enable class-methods-use-this */

export default FloatingPoint;