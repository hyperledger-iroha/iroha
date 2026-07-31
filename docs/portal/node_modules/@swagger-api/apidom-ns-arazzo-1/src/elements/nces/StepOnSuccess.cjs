"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class StepOnSuccess extends _apidomCore.ArrayElement {
  static primaryClass = 'step-on-success';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(StepOnSuccess.primaryClass);
  }
}
var _default = exports.default = StepOnSuccess;