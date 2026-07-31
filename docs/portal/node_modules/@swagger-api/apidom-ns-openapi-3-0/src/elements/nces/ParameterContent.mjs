import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ParameterContent extends ObjectElement {
  static primaryClass = 'parameter-content';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ParameterContent.primaryClass);
    this.classes.push('content');
  }
}
export default ParameterContent;