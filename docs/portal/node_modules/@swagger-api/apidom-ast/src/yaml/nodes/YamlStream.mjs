import Node from "../../Node.mjs";
import { isComment, isDocument } from "./predicates.mjs";
/**
 * @public
 */
class YamlStream extends Node {
  static type = 'stream';
}
Object.defineProperty(YamlStream.prototype, 'content', {
  get() {
    return Array.isArray(this.children) ? this.children.filter(node => isDocument(node) || isComment(node)) : [];
  },
  enumerable: true
});
export default YamlStream;