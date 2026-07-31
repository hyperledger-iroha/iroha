"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class EncodingHeaders extends _apidomCore.ObjectElement {
  static primaryClass = 'encoding-headers';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(EncodingHeaders.primaryClass);
  }
}
var _default = exports.default = EncodingHeaders;