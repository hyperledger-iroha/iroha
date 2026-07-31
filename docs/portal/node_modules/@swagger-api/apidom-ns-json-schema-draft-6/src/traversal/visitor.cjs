"use strict";

exports.__esModule = true;
exports.keyMap = exports.getNodeType = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-4");
exports.getNodeType = _apidomNsJsonSchemaDraft.getNodeType;
/**
 * @public
 */
const keyMap = exports.keyMap = {
  JSONSchemaDraft6Element: ['content'],
  JSONReferenceElement: ['content'],
  MediaElement: ['content'],
  LinkDescriptionElement: ['content'],
  ..._apidomCore.keyMap
};