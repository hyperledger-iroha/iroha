"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class StepParameters extends _apidomCore.ArrayElement {
  static primaryClass = 'step-parameters';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(StepParameters.primaryClass);
    this.classes.push('parameters');
  }
}
var _default = exports.default = StepParameters;