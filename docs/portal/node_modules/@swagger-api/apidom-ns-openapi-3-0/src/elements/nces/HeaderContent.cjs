"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class HeaderContent extends _apidomCore.ObjectElement {
  static primaryClass = 'header-content';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(HeaderContent.primaryClass);
    this.classes.push('content');
  }
}
var _default = exports.default = HeaderContent;