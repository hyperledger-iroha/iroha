"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _Node = _interopRequireDefault(require("../../Node.cjs"));
var _predicates = require("./predicates.cjs");
/**
 * @public
 */

/**
 * @public
 */
class YamlKeyValuePair extends _Node.default {
  static type = 'keyValuePair';
  styleGroup;
  constructor({
    styleGroup,
    ...rest
  }) {
    super({
      ...rest
    });
    this.styleGroup = styleGroup;
  }
}
Object.defineProperties(YamlKeyValuePair.prototype, {
  key: {
    get() {
      return this.children.filter(node => (0, _predicates.isScalar)(node) || (0, _predicates.isMapping)(node) || (0, _predicates.isSequence)(node))[0];
    },
    enumerable: true
  },
  value: {
    get() {
      const {
        key,
        children
      } = this;
      const excludeKeyPredicate = node => node !== key;
      const valuePredicate = node => (0, _predicates.isScalar)(node) || (0, _predicates.isMapping)(node) || (0, _predicates.isSequence)(node) || (0, _predicates.isAlias)(node);
      return children.filter(node => excludeKeyPredicate(node) && valuePredicate(node))[0];
    },
    enumerable: true
  }
});
var _default = exports.default = YamlKeyValuePair;