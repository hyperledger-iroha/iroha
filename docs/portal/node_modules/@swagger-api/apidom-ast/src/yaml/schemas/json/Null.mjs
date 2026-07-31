import Tag from "../Tag.mjs";
/* eslint-disable class-methods-use-this */
class Null extends Tag {
  static uri = 'tag:yaml.org,2002:null';
  test(node) {
    return /^null$/.test(node.content);
  }
  resolve(node) {
    const nodeClone = node.clone();
    nodeClone.content = null;
    return nodeClone;
  }
}
/* eslint-enable class-methods-use-this */

export default Null;