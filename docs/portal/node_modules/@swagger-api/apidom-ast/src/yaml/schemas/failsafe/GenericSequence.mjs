import Tag from "../Tag.mjs";
import { YamlNodeKind } from "../../nodes/YamlTag.mjs";
/* eslint-disable class-methods-use-this */
class GenericSequence extends Tag {
  static uri = 'tag:yaml.org,2002:seq';
  test(node) {
    return node.tag.kind === YamlNodeKind.Sequence;
  }
}
/* eslint-enable class-methods-use-this */

export default GenericSequence;