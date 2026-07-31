"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class SuccessAction extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'successAction';
  }
  get type() {
    return this.get('type');
  }
  set type(type) {
    this.set('type', type);
  }
  get workflowId() {
    return this.get('workflowId');
  }
  set workflowId(workflowId) {
    this.set('workflowId', workflowId);
  }
  get stepId() {
    return this.get('stepId');
  }
  set stepId(stepId) {
    this.set('stepId', stepId);
  }
  get criteria() {
    return this.get('criteria');
  }
  set criteria(criteria) {
    this.set('criteria', criteria);
  }
}
var _default = exports.default = SuccessAction;