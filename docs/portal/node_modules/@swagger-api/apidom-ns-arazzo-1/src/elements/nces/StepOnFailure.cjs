"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class StepOnFailure extends _apidomCore.ArrayElement {
  static primaryClass = 'step-on-failure';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(StepOnFailure.primaryClass);
  }
}
var _default = exports.default = StepOnFailure;