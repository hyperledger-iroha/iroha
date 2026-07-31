"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class OperationTraitSecurity extends _apidomCore.ArrayElement {
  static primaryClass = 'operation-trait-security';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(OperationTraitSecurity.primaryClass);
  }
}
var _default = exports.default = OperationTraitSecurity;