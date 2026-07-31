"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.UnsupportedOperationError = exports.NotImplementedError = exports.ApiDOMStructuredError = exports.ApiDOMError = exports.ApiDOMAggregateError = void 0;
var _ApiDOMError = _interopRequireDefault(require("./ApiDOMError.cjs"));
exports.ApiDOMError = _ApiDOMError.default;
var _ApiDOMAggregateError = _interopRequireDefault(require("./ApiDOMAggregateError.cjs"));
exports.ApiDOMAggregateError = _ApiDOMAggregateError.default;
var _ApiDOMStructuredError = _interopRequireDefault(require("./ApiDOMStructuredError.cjs"));
exports.ApiDOMStructuredError = _ApiDOMStructuredError.default;
var _UnsupportedOperationError = _interopRequireDefault(require("./UnsupportedOperationError.cjs"));
exports.UnsupportedOperationError = _UnsupportedOperationError.default;
var _NotImplementedError = _interopRequireDefault(require("./NotImplementedError.cjs"));
exports.NotImplementedError = _NotImplementedError.default;