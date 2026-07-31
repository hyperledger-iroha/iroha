import { formatFlowPlain, formatFlowSingleQuoted, formatFlowDoubleQuoted, formatBlockLiteral, formatBlockFolded } from "./canonical-format.mjs";
import { YamlStyle } from "../nodes/YamlStyle.mjs";
import { YamlNodeKind } from "../nodes/YamlTag.mjs";
class ScalarTag {
  static test(node) {
    return node.tag.kind === YamlNodeKind.Scalar && typeof node.content === 'string';
  }
  static canonicalFormat(node) {
    let canonicalForm = node.content;
    const nodeClone = node.clone();
    if (node.style === YamlStyle.Plain) {
      canonicalForm = formatFlowPlain(node.content);
    } else if (node.style === YamlStyle.SingleQuoted) {
      canonicalForm = formatFlowSingleQuoted(node.content);
    } else if (node.style === YamlStyle.DoubleQuoted) {
      canonicalForm = formatFlowDoubleQuoted(node.content);
    } else if (node.style === YamlStyle.Literal) {
      canonicalForm = formatBlockLiteral(node.content);
    } else if (node.style === YamlStyle.Folded) {
      canonicalForm = formatBlockFolded(node.content);
    }
    nodeClone.content = canonicalForm;
    return nodeClone;
  }
  static resolve(node) {
    return node;
  }
}
export default ScalarTag;