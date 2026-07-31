import { StringElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ArazzoSpec extends StringElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'arazzoSpec';
    this.classes.push('spec-version');
    this.classes.push('version');
  }
}
export default ArazzoSpec;