import FailsafeSchema from "../failsafe/index.mjs";
import BooleanTag from "./Boolean.mjs";
import FloatingPointTag from "./FloatingPoint.mjs";
import IntegerTag from "./Integer.mjs";
import NullTag from "./Null.mjs";
import { YamlNodeKind } from "../../nodes/YamlTag.mjs";
import GenericSequence from "../failsafe/GenericSequence.mjs";
import GenericMapping from "../failsafe/GenericMapping.mjs";
/**
 * @public
 */
class JsonSchema extends FailsafeSchema {
  constructor() {
    super();
    /**
     * We're registering more specific tags before more generic ones from Failsafe schema.
     */
    this.registerTag(new BooleanTag(), true);
    this.registerTag(new FloatingPointTag(), true);
    this.registerTag(new IntegerTag(), true);
    this.registerTag(new NullTag(), true);
  }
  toSpecificTagName(node) {
    let specificTagName = super.toSpecificTagName(node);
    if (specificTagName === '?') {
      if (node.tag.vkind === YamlNodeKind.Sequence) {
        specificTagName = GenericSequence.uri;
      } else if (node.tag.kind === YamlNodeKind.Mapping) {
        specificTagName = GenericMapping.uri;
      } else if (node.tag.kind === YamlNodeKind.Scalar) {
        const foundTag = this.tags.find(tag => tag.test(node));
        specificTagName = (foundTag === null || foundTag === void 0 ? void 0 : foundTag.tag) || '?';
      }
    }
    return specificTagName;
  }
}
export default JsonSchema;