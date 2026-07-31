"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _JsonNode = _interopRequireDefault(require("./JsonNode.cjs"));
var _predicates = require("./predicates.cjs");
/**
 * @public
 */

/**
 * @public
 */
class JsonProperty extends _JsonNode.default {
  static type = 'property';
  get key() {
    return this.children.find(_predicates.isKey);
  }
  get value() {
    return this.children.find(node => (0, _predicates.isFalse)(node) || (0, _predicates.isTrue)(node) || (0, _predicates.isNull)(node) || (0, _predicates.isNumber)(node) || (0, _predicates.isString)(node) || (0, _predicates.isArray)(node) || (0, _predicates.isObject)(node));
  }
}
var _default = exports.default = JsonProperty;