"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _YamlCollection = _interopRequireDefault(require("./YamlCollection.cjs"));
var _predicates = require("./predicates.cjs");
/**
 * @public
 */
class YamlSequence extends _YamlCollection.default {
  static type = 'sequence';
}
Object.defineProperty(YamlSequence.prototype, 'content', {
  get() {
    const {
      children
    } = this;
    return Array.isArray(children) ? children.filter(node => (0, _predicates.isSequence)(node) || (0, _predicates.isMapping)(node) || (0, _predicates.isScalar)(node) || (0, _predicates.isAlias)(node)) : [];
  },
  enumerable: true
});
var _default = exports.default = YamlSequence;