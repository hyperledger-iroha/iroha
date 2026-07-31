import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import MapVisitor from "../../generics/MapVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import WorkflowOutputsElement from "../../../../elements/nces/WorkflowOutputs.mjs";
/**
 * @public
 */
/**
 * @public
 */
class OutputsVisitor extends Mixin(MapVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new WorkflowOutputsElement();
    this.specPath = always(['value']);
  }
}
export default OutputsVisitor;