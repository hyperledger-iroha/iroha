"use strict";

exports.__esModule = true;
exports.keyMap = exports.getNodeType = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsJsonSchema = require("@swagger-api/apidom-ns-json-schema-2019-09");
exports.getNodeType = _apidomNsJsonSchema.getNodeType;
/**
 * @public
 */
const keyMap = exports.keyMap = {
  JSONSchema202012Element: ['content'],
  LinkDescriptionElement: ['content'],
  ..._apidomCore.keyMap
};