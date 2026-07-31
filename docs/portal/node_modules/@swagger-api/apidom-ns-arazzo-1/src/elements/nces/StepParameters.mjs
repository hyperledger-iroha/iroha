import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class StepParameters extends ArrayElement {
  static primaryClass = 'step-parameters';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(StepParameters.primaryClass);
    this.classes.push('parameters');
  }
}
export default StepParameters;