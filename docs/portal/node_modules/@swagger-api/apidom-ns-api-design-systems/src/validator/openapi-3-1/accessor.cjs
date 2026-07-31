"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-1");
/**
 * This file contains logic for translating Standard Identifier to value.
 */

const visitorOptions = {
  keyMap: _apidomNsOpenapi.keyMap,
  nodeTypeGetter: _apidomNsOpenapi.getNodeType
};
const access = (selected, standardIdentifier) => {
  const strStandardIdentifier = String((0, _apidomCore.toValue)(standardIdentifier));
  const values = new _apidomCore.ArrayElement();
  const visitor = {
    enter(element) {
      if (!element.meta.hasKey('ads-a-standard-identifier')) return;
      element.meta.get('ads-a-standard-identifier').content.filter(accessorMapping => {
        return String((0, _apidomCore.toValue)(accessorMapping.get('subject'))) === strStandardIdentifier;
      }).forEach(accessorMapping => {
        values.push(accessorMapping.get('value'));
      });
    }
  };
  (0, _apidomCore.visit)(selected, visitor, visitorOptions);
  return values;
};
var _default = exports.default = access;