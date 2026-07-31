"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class ResponseLinks extends _apidomCore.ObjectElement {
  static primaryClass = 'response-links';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ResponseLinks.primaryClass);
  }
}
var _default = exports.default = ResponseLinks;