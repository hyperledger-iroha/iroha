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
class YamlComment extends _Node.default {
  static type = 'comment';
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
var _default = exports.default = YamlComment;