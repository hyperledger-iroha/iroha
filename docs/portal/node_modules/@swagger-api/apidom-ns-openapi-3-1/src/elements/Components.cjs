"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
/**
 * @public
 */
class Components extends _apidomNsOpenapi.ComponentsElement {
  get pathItems() {
    return this.get('pathItems');
  }
  set pathItems(pathItems) {
    this.set('pathItems', pathItems);
  }
}
var _default = exports.default = Components;