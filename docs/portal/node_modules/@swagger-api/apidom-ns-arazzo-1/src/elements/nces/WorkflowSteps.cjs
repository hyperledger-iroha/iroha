"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class WorkflowSteps extends _apidomCore.ArrayElement {
  static primaryClass = 'workflow-steps';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(WorkflowSteps.primaryClass);
  }
}
var _default = exports.default = WorkflowSteps;