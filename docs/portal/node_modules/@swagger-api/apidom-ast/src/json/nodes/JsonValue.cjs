"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _JsonNode = _interopRequireDefault(require("./JsonNode.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class JsonValue extends _JsonNode.default {
  static type = 'value';
  value;
  constructor({
    value,
    ...rest
  }) {
    super({
      ...rest
    });
    this.value = value;
  }
}
var _default = exports.default = JsonValue;