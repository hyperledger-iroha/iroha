"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _JsonNode = _interopRequireDefault(require("./JsonNode.cjs"));
var _predicates = require("./predicates.cjs");
/**
 * @public
 */
class JsonString extends _JsonNode.default {
  static type = 'string';
  get value() {
    if (this.children.length === 1) {
      const onlyChild = this.children[0];
      return onlyChild.value;
    }
    return this.children.filter(node => (0, _predicates.isStringContent)(node) || (0, _predicates.isEscapeSequence)(node)).reduce((acc, cur) => acc + cur.value, '');
  }
}
var _default = exports.default = JsonString;