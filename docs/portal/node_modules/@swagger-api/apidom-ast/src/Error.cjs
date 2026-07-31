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
class Error extends _Node.default {
  static type = 'error';
  value;
  isUnexpected;
  constructor({
    value,
    isUnexpected = false,
    ...rest
  } = {}) {
    super({
      ...rest
    });
    this.value = value;
    this.isUnexpected = isUnexpected;
  }
}
var _default = exports.default = Error;