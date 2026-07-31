"use strict";

exports.__esModule = true;
exports.keyMap = exports.getNodeType = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
const getNodeType = element => {
  if (!(0, _apidomCore.isElement)(element)) {
    return undefined;
  }
  return `${element.element.charAt(0).toUpperCase() + element.element.slice(1)}Element`;
};

/**
 * @public
 */
exports.getNodeType = getNodeType;
const keyMap = exports.keyMap = {
  JSONSchemaDraft4Element: ['content'],
  JSONReferenceElement: ['content'],
  MediaElement: ['content'],
  LinkDescriptionElement: ['content'],
  ..._apidomCore.keyMap
};