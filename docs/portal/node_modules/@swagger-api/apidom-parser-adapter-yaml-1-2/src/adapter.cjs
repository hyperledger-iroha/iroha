"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.namespace = exports.mediaTypes = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _mediaTypes = _interopRequireDefault(require("./media-types.cjs"));
exports.mediaTypes = _mediaTypes.default;
/**
 * @public
 */
const namespace = exports.namespace = (0, _apidomCore.createNamespace)();