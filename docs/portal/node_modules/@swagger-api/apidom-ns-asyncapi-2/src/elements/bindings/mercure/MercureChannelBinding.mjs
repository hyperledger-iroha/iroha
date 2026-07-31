import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class MercureChannelBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'mercureChannelBinding';
    this.classes.push('channel-binding');
  }
}
export default MercureChannelBinding;