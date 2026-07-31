import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Discriminator extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'discriminator';
  }
  get propertyName() {
    return this.get('propertyName');
  }
  set propertyName(propertyName) {
    this.set('propertyName', propertyName);
  }
  get mapping() {
    return this.get('mapping');
  }
  set mapping(mapping) {
    this.set('mapping', mapping);
  }
}
export default Discriminator;