"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class StepSuccessCriteria extends _apidomCore.ArrayElement {
  static primaryClass = 'step-success-criteria';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(StepSuccessCriteria.primaryClass);
    this.classes.push('criteria');
  }
}
var _default = exports.default = StepSuccessCriteria;