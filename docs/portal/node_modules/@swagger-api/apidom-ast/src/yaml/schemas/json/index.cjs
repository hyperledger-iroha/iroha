"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _index = _interopRequireDefault(require("../failsafe/index.cjs"));
var _Boolean = _interopRequireDefault(require("./Boolean.cjs"));
var _FloatingPoint = _interopRequireDefault(require("./FloatingPoint.cjs"));
var _Integer = _interopRequireDefault(require("./Integer.cjs"));
var _Null = _interopRequireDefault(require("./Null.cjs"));
var _YamlTag = require("../../nodes/YamlTag.cjs");
var _GenericSequence = _interopRequireDefault(require("../failsafe/GenericSequence.cjs"));
var _GenericMapping = _interopRequireDefault(require("../failsafe/GenericMapping.cjs"));
/**
 * @public
 */
class JsonSchema extends _index.default {
  constructor() {
    super();
    /**
     * We're registering more specific tags before more generic ones from Failsafe schema.
     */
    this.registerTag(new _Boolean.default(), true);
    this.registerTag(new _FloatingPoint.default(), true);
    this.registerTag(new _Integer.default(), true);
    this.registerTag(new _Null.default(), true);
  }
  toSpecificTagName(node) {
    let specificTagName = super.toSpecificTagName(node);
    if (specificTagName === '?') {
      if (node.tag.vkind === _YamlTag.YamlNodeKind.Sequence) {
        specificTagName = _GenericSequence.default.uri;
      } else if (node.tag.kind === _YamlTag.YamlNodeKind.Mapping) {
        specificTagName = _GenericMapping.default.uri;
      } else if (node.tag.kind === _YamlTag.YamlNodeKind.Scalar) {
        const foundTag = this.tags.find(tag => tag.test(node));
        specificTagName = (foundTag == null ? void 0 : foundTag.tag) || '?';
      }
    }
    return specificTagName;
  }
}
var _default = exports.default = JsonSchema;