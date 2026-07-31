"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
/**
 * @public
 */
const jsonMediaTypes = new _apidomNsOpenapi.OpenAPIMediaTypes(..._apidomNsOpenapi.mediaTypes.filterByFormat('generic'), ..._apidomNsOpenapi.mediaTypes.filterByFormat('json'));
var _default = exports.default = jsonMediaTypes;