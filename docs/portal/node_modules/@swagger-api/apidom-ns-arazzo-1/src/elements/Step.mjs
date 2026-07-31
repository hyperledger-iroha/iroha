import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Step extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'step';
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
  get stepId() {
    return this.get('stepId');
  }
  set stepId(stepId) {
    this.set('stepId', stepId);
  }
  get operationId() {
    return this.get('operationId');
  }
  set operationId(operationId) {
    this.set('operationId', operationId);
  }
  get operationRef() {
    return this.get('operationRef');
  }
  set operationRef(operationRef) {
    this.set('operationRef', operationRef);
  }
  get workflowId() {
    return this.get('workflowId');
  }
  set workflowId(workflowId) {
    this.set('workflowId', workflowId);
  }
  get parameters() {
    return this.get('parameters');
  }
  set parameters(parameters) {
    this.set('parameters', parameters);
  }
  get dependsOn() {
    return this.get('dependsOn');
  }
  set dependsOn(dependsOn) {
    this.set('dependsOn', dependsOn);
  }
  get successCriteria() {
    return this.get('successCriteria');
  }
  set successCriteria(successCriteria) {
    this.set('successCriteria', successCriteria);
  }
  get onSuccess() {
    return this.get('onSuccess');
  }
  set onSuccess(onSuccess) {
    this.set('onSuccess', onSuccess);
  }
  get onFailure() {
    return this.get('onFailure');
  }
  set onFailure(onFailure) {
    this.set('onFailure', onFailure);
  }
  get outputs() {
    return this.get('outputs');
  }
  set outputs(outputs) {
    this.set('outputs', outputs);
  }
}
export default Step;