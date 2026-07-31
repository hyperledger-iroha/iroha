"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _JsonValue = _interopRequireDefault(require("./JsonValue.cjs"));
/**
 * @public
 */
class JsonNull extends _JsonValue.default {
  static type = 'null';
}
var _default = exports.default = JsonNull;