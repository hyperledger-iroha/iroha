"use strict";

exports.__esModule = true;
exports.keyMap = exports.getNodeType = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-6");
exports.getNodeType = _apidomNsJsonSchemaDraft.getNodeType;
/**
 * @public
 */
const keyMap = exports.keyMap = {
  JSONSchemaDraft7Element: ['content'],
  JSONReferenceElement: ['content'],
  LinkDescriptionElement: ['content'],
  ..._apidomCore.keyMap
};