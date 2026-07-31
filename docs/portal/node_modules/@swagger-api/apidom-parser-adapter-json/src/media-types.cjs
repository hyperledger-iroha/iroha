"use strict";

exports.__esModule = true;
exports.default = exports.JSONMediaTypes = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class JSONMediaTypes extends _apidomCore.MediaTypes {
  latest() {
    return this[0];
  }
}

/**
 * @public
 */
exports.JSONMediaTypes = JSONMediaTypes;
const mediaTypes = new JSONMediaTypes('application/json');
var _default = exports.default = mediaTypes;