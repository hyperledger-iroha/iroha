import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class StepOutputs extends ObjectElement {
  static primaryClass = 'step-outputs';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(StepOutputs.primaryClass);
  }
}
export default StepOutputs;