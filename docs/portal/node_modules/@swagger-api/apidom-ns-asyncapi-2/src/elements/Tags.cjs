"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class Tags extends _apidomCore.ArrayElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'tags';
  }
}
var _default = exports.default = Tags;