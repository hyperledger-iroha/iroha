import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class StepOnSuccess extends ArrayElement {
  static primaryClass = 'step-on-success';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(StepOnSuccess.primaryClass);
  }
}
export default StepOnSuccess;