import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class MediaTypeEncoding extends ObjectElement {
  static primaryClass = 'media-type-encoding';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(MediaTypeEncoding.primaryClass);
  }
}
export default MediaTypeEncoding;