"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _Node = _interopRequireDefault(require("../../Node.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class YamlNode extends _Node.default {
  anchor;
  tag;
  style;
  styleGroup;
  constructor({
    anchor,
    tag,
    style,
    styleGroup,
    ...rest
  }) {
    super({
      ...rest
    });
    this.anchor = anchor;
    this.tag = tag;
    this.style = style;
    this.styleGroup = styleGroup;
  }
}
var _default = exports.default = YamlNode;