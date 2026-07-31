"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class DefaultContentType extends _apidomCore.StringElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'defaultContentType';
  }
}
var _default = exports.default = DefaultContentType;