import { StringElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class SwaggerVersion extends StringElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'swaggerVersion';
    this.classes.push('spec-version');
    this.classes.push('version');
  }
}
export default SwaggerVersion;