"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _Node = _interopRequireDefault(require("../../Node.cjs"));
var _predicates = require("./predicates.cjs");
/**
 * @public
 */
class YamlStream extends _Node.default {
  static type = 'stream';
}
Object.defineProperty(YamlStream.prototype, 'content', {
  get() {
    return Array.isArray(this.children) ? this.children.filter(node => (0, _predicates.isDocument)(node) || (0, _predicates.isComment)(node)) : [];
  },
  enumerable: true
});
var _default = exports.default = YamlStream;