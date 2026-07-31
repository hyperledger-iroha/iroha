"use strict";

exports.__esModule = true;
exports.default = void 0;
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-1");
/**
 * This file contains logic for translating Standard Identifier to list of Operation Elements.
 */

const visitorOptions = {
  keyMap: _apidomNsOpenapi.keyMap,
  nodeTypeGetter: _apidomNsOpenapi.getNodeType
};
const select = (openAPIElement, standardIdentifier) => {
  const selected = [];
  const visitor = {
    OperationElement(element) {
      if (!element.meta.hasKey('ads-s-standard-identifier')) return;
      const standardIdentifiers = (0, _apidomCore.toValue)(element.meta.get('ads-s-standard-identifier'));
      if ((0, _ramda.includes)((0, _apidomCore.toValue)(standardIdentifier), standardIdentifiers)) {
        selected.push(element);
      }
    },
    ResponseElement(element) {
      if (!element.meta.hasKey('ads-s-standard-identifier')) return;
      const standardIdentifiers = (0, _apidomCore.toValue)(element.meta.get('ads-s-standard-identifier'));
      if ((0, _ramda.includes)((0, _apidomCore.toValue)(standardIdentifier), standardIdentifiers)) {
        selected.push(element);
      }
    }
  };
  (0, _apidomCore.visit)(openAPIElement, visitor, visitorOptions);
  return selected;
};
var _default = exports.default = select;