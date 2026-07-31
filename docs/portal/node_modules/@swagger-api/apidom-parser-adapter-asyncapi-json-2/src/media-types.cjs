"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
/**
 * @public
 */
const jsonMediaTypes = new _apidomNsAsyncapi.AsyncAPIMediaTypes(..._apidomNsAsyncapi.mediaTypes.filterByFormat('generic'), ..._apidomNsAsyncapi.mediaTypes.filterByFormat('json'));
var _default = exports.default = jsonMediaTypes;