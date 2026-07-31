"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class FailureActionCriteria extends _apidomCore.ArrayElement {
  static primaryClass = 'failure-action-criteria';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(FailureActionCriteria.primaryClass);
    this.classes.push('criteria');
  }
}
var _default = exports.default = FailureActionCriteria;