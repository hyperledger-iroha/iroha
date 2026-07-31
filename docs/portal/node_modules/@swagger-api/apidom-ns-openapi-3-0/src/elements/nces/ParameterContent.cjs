"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class ParameterContent extends _apidomCore.ObjectElement {
  static primaryClass = 'parameter-content';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ParameterContent.primaryClass);
    this.classes.push('content');
  }
}
var _default = exports.default = ParameterContent;