import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class SwaggerProduces extends ArrayElement {
  static primaryClass = 'swagger-produces';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(SwaggerProduces.primaryClass);
  }
}
export default SwaggerProduces;