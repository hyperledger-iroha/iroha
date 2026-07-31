import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import WebSocketServerBindingElement from "../../../../../../elements/bindings/ws/WebSocketServerBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class WebSocketServerBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new WebSocketServerBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'ws', 'ServerBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default WebSocketServerBindingVisitor;