import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Response extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'response';
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
  get headers() {
    return this.get('headers');
  }
  set headers(headers) {
    this.set('headers', headers);
  }
  get contentProp() {
    return this.get('content');
  }
  set contentProp(contentProp) {
    this.set('content', contentProp);
  }
  get links() {
    return this.get('links');
  }
  set links(links) {
    this.set('links', links);
  }
}
export default Response;