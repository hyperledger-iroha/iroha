import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ParameterExamples extends ObjectElement {
  static primaryClass = 'parameter-examples';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ParameterExamples.primaryClass);
    this.classes.push('examples');
  }
}
export default ParameterExamples;