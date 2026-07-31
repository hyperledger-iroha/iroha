import Tag from "../Tag.mjs";
import { YamlNodeKind } from "../../nodes/YamlTag.mjs";
/* eslint-disable class-methods-use-this */
class GenericMapping extends Tag {
  static uri = 'tag:yaml.org,2002:map';
  test(node) {
    return node.tag.kind === YamlNodeKind.Mapping;
  }
}
/* eslint-enable class-methods-use-this */

export default GenericMapping;