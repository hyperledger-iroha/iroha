import { head } from 'ramda';
import Node from "./Node.mjs";
/**
 * @public
 */
class ParseResult extends Node {
  static type = 'parseResult';
  get rootNode() {
    return head(this.children);
  }
}
export default ParseResult;