"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ramda = require("ramda");
var _Node = _interopRequireDefault(require("./Node.cjs"));
/**
 * @public
 */
class ParseResult extends _Node.default {
  static type = 'parseResult';
  get rootNode() {
    return (0, _ramda.head)(this.children);
  }
}
var _default = exports.default = ParseResult;