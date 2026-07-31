"use strict";

exports.__esModule = true;
exports.default = void 0;
var _canonicalFormat = require("./canonical-format.cjs");
var _YamlStyle = require("../nodes/YamlStyle.cjs");
var _YamlTag = require("../nodes/YamlTag.cjs");
class ScalarTag {
  static test(node) {
    return node.tag.kind === _YamlTag.YamlNodeKind.Scalar && typeof node.content === 'string';
  }
  static canonicalFormat(node) {
    let canonicalForm = node.content;
    const nodeClone = node.clone();
    if (node.style === _YamlStyle.YamlStyle.Plain) {
      canonicalForm = (0, _canonicalFormat.formatFlowPlain)(node.content);
    } else if (node.style === _YamlStyle.YamlStyle.SingleQuoted) {
      canonicalForm = (0, _canonicalFormat.formatFlowSingleQuoted)(node.content);
    } else if (node.style === _YamlStyle.YamlStyle.DoubleQuoted) {
      canonicalForm = (0, _canonicalFormat.formatFlowDoubleQuoted)(node.content);
    } else if (node.style === _YamlStyle.YamlStyle.Literal) {
      canonicalForm = (0, _canonicalFormat.formatBlockLiteral)(node.content);
    } else if (node.style === _YamlStyle.YamlStyle.Folded) {
      canonicalForm = (0, _canonicalFormat.formatBlockFolded)(node.content);
    }
    nodeClone.content = canonicalForm;
    return nodeClone;
  }
  static resolve(node) {
    return node;
  }
}
var _default = exports.default = ScalarTag;