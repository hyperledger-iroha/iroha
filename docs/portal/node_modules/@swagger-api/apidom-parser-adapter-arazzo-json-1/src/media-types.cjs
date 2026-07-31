"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomNsArazzo = require("@swagger-api/apidom-ns-arazzo-1");
/**
 * @public
 */
const jsonMediaTypes = new _apidomNsArazzo.ArazzoMediaTypes(..._apidomNsArazzo.mediaTypes.filterByFormat('generic'), ..._apidomNsArazzo.mediaTypes.filterByFormat('json'));
var _default = exports.default = jsonMediaTypes;