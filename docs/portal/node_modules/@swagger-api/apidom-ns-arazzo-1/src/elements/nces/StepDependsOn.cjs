"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class StepDependsOn extends _apidomCore.ArrayElement {
  static primaryClass = 'step-depends-on';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(StepDependsOn.primaryClass);
  }
}
var _default = exports.default = StepDependsOn;