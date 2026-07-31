"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _JsonNode = _interopRequireDefault(require("./JsonNode.cjs"));
var _predicates = require("./predicates.cjs");
/**
 * @public
 */
class JsonObject extends _JsonNode.default {
  static type = 'object';
  get properties() {
    return this.children.filter(_predicates.isProperty);
  }
}
var _default = exports.default = JsonObject;