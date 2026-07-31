"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomNsApiDesignSystems = require("@swagger-api/apidom-ns-api-design-systems");
/**
 * @public
 */
const yamlMediaTypes = new _apidomNsApiDesignSystems.ApiDesignSystemsMediaTypes(..._apidomNsApiDesignSystems.mediaTypes.filterByFormat('generic'), ..._apidomNsApiDesignSystems.mediaTypes.filterByFormat('yaml'));
var _default = exports.default = yamlMediaTypes;