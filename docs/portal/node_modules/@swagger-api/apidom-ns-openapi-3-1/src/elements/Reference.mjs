import { ReferenceElement } from '@swagger-api/apidom-ns-openapi-3-0';

/**
 * @public
 */
class Reference extends ReferenceElement {}
Object.defineProperty(Reference.prototype, 'description', {
  get() {
    return this.get('description');
  },
  set(description) {
    this.set('description', description);
  },
  enumerable: true
});
Object.defineProperty(Reference.prototype, 'summary', {
  get() {
    return this.get('summary');
  },
  set(description) {
    this.set('summary', description);
  },
  enumerable: true
});
export default Reference;