"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _Node = _interopRequireDefault(require("./Node.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class Literal extends _Node.default {
  static type = 'literal';
  value;
  constructor({
    value,
    ...rest
  } = {}) {
    super({
      ...rest
    });
    this.value = value;
  }
}
var _default = exports.default = Literal;