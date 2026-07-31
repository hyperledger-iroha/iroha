"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.namespace = exports.mediaTypes = exports.detectionRegExp = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _mediaTypes = _interopRequireDefault(require("./media-types.cjs"));
exports.mediaTypes = _mediaTypes.default;
/**
 * @public
 */
const namespace = exports.namespace = (0, _apidomCore.createNamespace)();

/**
 * @public
 */
const detectionRegExp = exports.detectionRegExp =
// eslint-disable-next-line no-control-regex
/(?<true>^\s*true\s*$)|(?<false>^\s*false\s*$)|(?<null>^\s*null\s*$)|(?<number>^\s*\d+\s*$)|(?<object>^\s*{\s*)|(?<array>^\s*\[\s*)|(?<string>^\s*"(((?=\\)\\(["\\/bfnrt]|u[0-9a-fA-F]{4}))|[^"\\\x00-\x1F\x7F])*"\s*$)/;