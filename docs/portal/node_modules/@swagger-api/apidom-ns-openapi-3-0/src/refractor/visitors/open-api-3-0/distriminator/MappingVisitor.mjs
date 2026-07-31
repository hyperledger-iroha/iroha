import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import MapVisitor from "../../generics/MapVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import DiscriminatorMappingElement from "../../../../elements/nces/DiscriminatorMapping.mjs";
/**
 * @public
 */
/**
 * @public
 */
class MappingVisitor extends Mixin(MapVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new DiscriminatorMappingElement();
    this.specPath = always(['value']);
  }
}
export default MappingVisitor;