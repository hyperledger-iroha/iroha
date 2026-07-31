import Tag from "../Tag.mjs";
/* eslint-disable class-methods-use-this */
class Integer extends Tag {
  static uri = 'tag:yaml.org,2002:int';
  test(node) {
    return /^-?(0|[1-9][0-9]*)$/.test(node.content);
  }
  resolve(node) {
    const content = parseInt(node.content, 10);
    const nodeClone = node.clone();
    nodeClone.content = content;
    return nodeClone;
  }
}
/* eslint-enable class-methods-use-this */

export default Integer;