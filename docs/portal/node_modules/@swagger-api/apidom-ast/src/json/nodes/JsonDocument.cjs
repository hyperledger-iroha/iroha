"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ramda = require("ramda");
var _JsonNode = _interopRequireDefault(require("./JsonNode.cjs"));
/**
 * @public
 */
class JsonDocument extends _JsonNode.default {
  static type = 'document';
  get child() {
    return (0, _ramda.head)(this.children);
  }
}
var _default = exports.default = JsonDocument;