import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Channels extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'channels';
  }
}
export default Channels;