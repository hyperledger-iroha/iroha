import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import { ObjectElement } from '@swagger-api/apidom-core';
import MapVisitor from "../generics/MapVisitor.mjs";
import FallbackVisitor from "../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ContentVisitor extends Mixin(MapVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ObjectElement();
    this.element.classes.push('content');
    this.specPath = always(['document', 'objects', 'MediaType']);
  }
}
export default ContentVisitor;