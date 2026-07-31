import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class StepOnFailure extends ArrayElement {
  static primaryClass = 'step-on-failure';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(StepOnFailure.primaryClass);
  }
}
export default StepOnFailure;