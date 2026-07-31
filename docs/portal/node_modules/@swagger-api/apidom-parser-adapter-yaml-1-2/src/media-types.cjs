"use strict";

exports.__esModule = true;
exports.default = exports.YamlMediaTypes = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class YamlMediaTypes extends _apidomCore.MediaTypes {
  latest() {
    return this[1];
  }
}

/**
 * @public
 */
exports.YamlMediaTypes = YamlMediaTypes;
const mediaTypes = new YamlMediaTypes('text/yaml', 'application/yaml');
var _default = exports.default = mediaTypes;