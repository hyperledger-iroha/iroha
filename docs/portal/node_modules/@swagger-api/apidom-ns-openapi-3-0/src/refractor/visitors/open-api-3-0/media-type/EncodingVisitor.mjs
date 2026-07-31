import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import MediaTypeEncodingElement from "../../../../elements/nces/MediaTypeEncoding.mjs";
import MapVisitor from "../../generics/MapVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class EncodingVisitor extends Mixin(MapVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new MediaTypeEncodingElement();
    this.specPath = always(['document', 'objects', 'Encoding']);
  }
}
export default EncodingVisitor;