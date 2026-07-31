"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _YamlNode = _interopRequireDefault(require("./YamlNode.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class YamlScalar extends _YamlNode.default {
  static type = 'scalar';
  content;
  constructor({
    content,
    ...rest
  }) {
    super({
      ...rest
    });
    this.content = content;
  }
}
var _default = exports.default = YamlScalar;