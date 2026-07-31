import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class SwaggerSecurity extends ArrayElement {
  static primaryClass = 'swagger-security';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(SwaggerSecurity.primaryClass);
    this.classes.push('security');
  }
}
export default SwaggerSecurity;