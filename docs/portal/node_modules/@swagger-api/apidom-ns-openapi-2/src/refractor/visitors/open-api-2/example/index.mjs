import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import ExampleElement from "../../../../elements/Example.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import MapVisitor from "../../generics/MapVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ExampleVisitor extends Mixin(MapVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ExampleElement();
    this.specPath = always(['value']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default ExampleVisitor;