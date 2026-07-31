"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.YamlNodeKind = void 0;
var _Node = _interopRequireDefault(require("../../Node.cjs"));
/**
 * @public
 */
let YamlNodeKind = exports.YamlNodeKind = /*#__PURE__*/function (YamlNodeKind) {
  YamlNodeKind["Scalar"] = "Scalar";
  YamlNodeKind["Sequence"] = "Sequence";
  YamlNodeKind["Mapping"] = "Mapping";
  return YamlNodeKind;
}({});
/**
 * @public
 */
/**
 * @public
 */
class YamlTag extends _Node.default {
  static type = 'tag';
  explicitName;
  kind;
  constructor({
    explicitName,
    kind,
    ...rest
  }) {
    super({
      ...rest
    });
    this.explicitName = explicitName;
    this.kind = kind;
  }
}
var _default = exports.default = YamlTag;