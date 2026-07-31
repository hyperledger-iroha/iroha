import { StringElement } from 'minim';

/**
 * @public
 */
class Annotation extends StringElement {
  // classes: warning | error

  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'annotation';
  }
  get code() {
    return this.attributes.get('code');
  }
  set code(value) {
    this.attributes.set('code', value);
  }
}
export default Annotation;