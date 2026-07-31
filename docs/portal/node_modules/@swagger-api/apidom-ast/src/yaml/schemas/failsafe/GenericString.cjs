"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _Tag = _interopRequireDefault(require("../Tag.cjs"));
class GenericString extends _Tag.default {
  static uri = 'tag:yaml.org,2002:str';
}
var _default = exports.default = GenericString;