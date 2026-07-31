"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class Tags extends _apidomCore.ArrayElement {
  static primaryClass = 'tags';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(Tags.primaryClass);
  }
}
var _default = exports.default = Tags;