import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class DiscriminatorMapping extends ObjectElement {
  static primaryClass = 'discriminator-mapping';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(DiscriminatorMapping.primaryClass);
  }
}
export default DiscriminatorMapping;