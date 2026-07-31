import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class HeaderContent extends ObjectElement {
  static primaryClass = 'header-content';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(HeaderContent.primaryClass);
    this.classes.push('content');
  }
}
export default HeaderContent;