"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class StepOutputs extends _apidomCore.ObjectElement {
  static primaryClass = 'step-outputs';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(StepOutputs.primaryClass);
  }
}
var _default = exports.default = StepOutputs;