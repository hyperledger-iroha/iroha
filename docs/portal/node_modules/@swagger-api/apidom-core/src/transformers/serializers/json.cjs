"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _index = _interopRequireDefault(require("./value/index.cjs"));
/**
 * @public
 */
const serializer = (element, replacer, space) => JSON.stringify((0, _index.default)(element), replacer, space);
var _default = exports.default = serializer;