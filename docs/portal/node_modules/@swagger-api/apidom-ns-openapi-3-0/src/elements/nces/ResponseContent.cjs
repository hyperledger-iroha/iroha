"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class ResponseContent extends _apidomCore.ObjectElement {
  static primaryClass = 'response-content';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ResponseContent.primaryClass);
    this.classes.push('content');
  }
}
var _default = exports.default = ResponseContent;