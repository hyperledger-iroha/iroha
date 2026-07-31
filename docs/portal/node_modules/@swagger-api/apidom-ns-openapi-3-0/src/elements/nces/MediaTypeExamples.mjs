import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class MediaTypeExamples extends ObjectElement {
  static primaryClass = 'media-type-examples';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(MediaTypeExamples.primaryClass);
    this.classes.push('examples');
  }
}
export default MediaTypeExamples;