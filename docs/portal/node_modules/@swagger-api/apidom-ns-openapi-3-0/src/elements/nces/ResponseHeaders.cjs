"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class ResponseHeaders extends _apidomCore.ObjectElement {
  static primaryClass = 'response-headers';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ResponseHeaders.primaryClass);
  }
}
var _default = exports.default = ResponseHeaders;