import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import ResponsesDefinitionsElement from "../../../../elements/ResponsesDefinitions.mjs";
import MapVisitor from "../../generics/MapVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ResponsesDefinitionsVisitor extends Mixin(MapVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ResponsesDefinitionsElement();
    this.specPath = always(['document', 'objects', 'Response']);
  }
}
export default ResponsesDefinitionsVisitor;