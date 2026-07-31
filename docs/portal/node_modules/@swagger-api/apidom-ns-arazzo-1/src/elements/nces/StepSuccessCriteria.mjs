import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class StepSuccessCriteria extends ArrayElement {
  static primaryClass = 'step-success-criteria';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(StepSuccessCriteria.primaryClass);
    this.classes.push('criteria');
  }
}
export default StepSuccessCriteria;