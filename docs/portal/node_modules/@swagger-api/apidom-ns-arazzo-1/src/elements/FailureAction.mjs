import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class FailureAction extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'failureAction';
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
  get retryAfter() {
    return this.get('retryAfter');
  }
  set retryAfter(retryAfter) {
    this.set('retryAfter', retryAfter);
  }
  get retryLimit() {
    return this.get('retryLimit');
  }
  set retryLimit(retryLimit) {
    this.set('retryLimit', retryLimit);
  }
  get criteria() {
    return this.get('criteria');
  }
  set criteria(criteria) {
    this.set('criteria', criteria);
  }
}
export default FailureAction;