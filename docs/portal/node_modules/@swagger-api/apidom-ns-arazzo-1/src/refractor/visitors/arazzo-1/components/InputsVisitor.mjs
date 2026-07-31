import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import ComponentsInputsElement from "../../../../elements/nces/ComponentsInputs.mjs";
import MapVisitor from "../../generics/MapVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class InputsVisitor extends Mixin(MapVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ComponentsInputsElement();
    this.specPath = always(['document', 'objects', 'JSONSchema']);
  }
}
export default InputsVisitor;