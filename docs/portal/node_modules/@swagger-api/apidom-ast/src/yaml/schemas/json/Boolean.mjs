import Tag from "../Tag.mjs";
/* eslint-disable class-methods-use-this */
class Boolean extends Tag {
  static uri = 'tag:yaml.org,2002:bool';
  test(node) {
    return /^(true|false)$/.test(node.content);
  }
  resolve(node) {
    const content = node.content === 'true';
    const nodeClone = node.clone();
    nodeClone.content = content;
    return nodeClone;
  }
}
/* eslint-enable class-methods-use-this */

export default Boolean;