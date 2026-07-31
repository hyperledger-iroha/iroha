import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class CorrelationID extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'correlationID';
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
  get location() {
    return this.get('location');
  }
  set location(location) {
    this.set('location', location);
  }
}
export default CorrelationID;