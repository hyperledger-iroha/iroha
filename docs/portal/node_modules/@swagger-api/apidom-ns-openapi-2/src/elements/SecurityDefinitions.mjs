import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class SecurityDefinitions extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'securityDefinitions';
  }
}
export default SecurityDefinitions;