"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class Workflow extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'workflow';
  }
  get workflowId() {
    return this.get('workflowId');
  }
  set workflowId(workflowId) {
    this.set('workflowId', workflowId);
  }
  get summary() {
    return this.get('summary');
  }
  set summary(summary) {
    this.set('summary', summary);
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
  get inputs() {
    return this.get('inputs');
  }
  set inputs(inputs) {
    this.set('inputs', inputs);
  }
  get steps() {
    return this.get('steps');
  }
  set steps(steps) {
    this.set('steps', steps);
  }
  get outputs() {
    return this.get('outputs');
  }
  set outputs(outputs) {
    this.set('outputs', outputs);
  }
}
var _default = exports.default = Workflow;