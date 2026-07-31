"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class OperationSchemes extends _apidomCore.ArrayElement {
  static primaryClass = 'operation-schemes';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(OperationSchemes.primaryClass);
  }
}
var _default = exports.default = OperationSchemes;