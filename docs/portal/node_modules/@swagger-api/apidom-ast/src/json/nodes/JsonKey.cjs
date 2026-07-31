"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _JsonString = _interopRequireDefault(require("./JsonString.cjs"));
/**
 * @public
 */
class JsonKey extends _JsonString.default {
  static type = 'key';
}
var _default = exports.default = JsonKey;