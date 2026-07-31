import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class HeaderExamples extends ObjectElement {
  static primaryClass = 'header-examples';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(HeaderExamples.primaryClass);
    this.classes.push('examples');
  }
}
export default HeaderExamples;