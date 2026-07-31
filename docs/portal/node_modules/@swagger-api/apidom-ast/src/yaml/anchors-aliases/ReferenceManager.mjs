import YamlScalar from "../nodes/YamlScalar.mjs";
import YamlReferenceError from "../errors/YamlReferenceError.mjs";
import { isAnchor } from "../nodes/predicates.mjs";
import { YamlStyle, YamlStyleGroup } from "../nodes/YamlStyle.mjs";
/* eslint-disable class-methods-use-this */
/**
 * @public
 */
class ReferenceManager {
  addAnchor(node) {
    if (!isAnchor(node.anchor)) {
      throw new YamlReferenceError('Expected YAML anchor to be attached the the YAML AST node.', {
        node
      });
    }
  }
  resolveAlias(alias) {
    return new YamlScalar({
      content: alias.content,
      style: YamlStyle.Plain,
      styleGroup: YamlStyleGroup.Flow
    });
  }
}
/* eslint-enable class-methods-use-this */

export default ReferenceManager;