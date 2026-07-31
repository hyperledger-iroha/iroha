"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class RequestBodyContent extends _apidomCore.ObjectElement {
  static primaryClass = 'request-body-content';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(RequestBodyContent.primaryClass);
    this.classes.push('content');
  }
}
var _default = exports.default = RequestBodyContent;