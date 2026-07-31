import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import WebSocketOperationBindingElement from "../../../../../../elements/bindings/ws/WebSocketOperationBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class WebSocketOperationBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new WebSocketOperationBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'ws', 'OperationBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default WebSocketOperationBindingVisitor;