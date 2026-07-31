import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Encoding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'encoding';
  }
  get contentType() {
    return this.get('contentType');
  }
  set contentType(contentType) {
    this.set('contentType', contentType);
  }
  get headers() {
    return this.get('headers');
  }
  set headers(headers) {
    this.set('headers', headers);
  }
  get style() {
    return this.get('style');
  }
  set style(style) {
    this.set('style', style);
  }
  get explode() {
    return this.get('explode');
  }
  set explode(explode) {
    this.set('explode', explode);
  }
  get allowedReserved() {
    return this.get('allowedReserved');
  }
  set allowedReserved(allowedReserved) {
    this.set('allowedReserved', allowedReserved);
  }
}
export default Encoding;