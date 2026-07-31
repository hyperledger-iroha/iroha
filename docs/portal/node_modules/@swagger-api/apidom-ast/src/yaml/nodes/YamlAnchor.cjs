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
class YamlAnchor extends _Node.default {
  static type = 'anchor';
  name;
  constructor({
    name,
    ...rest
  }) {
    super({
      ...rest
    });
    this.name = name;
  }
}
var _default = exports.default = YamlAnchor;