import { Mixin } from 'ts-mixer';
import { isStringElement } from '@swagger-api/apidom-core';
import { always } from 'ramda';
import OperationElement from "../../../../elements/Operation.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
class OperationVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new OperationElement();
    this.specPath = always(['document', 'objects', 'Operation']);
    this.canSupportSpecificationExtensions = true;
  }
  ObjectElement(objectElement) {
    const result = FixedFieldsVisitor.prototype.ObjectElement.call(this, objectElement);
    const action = objectElement.get('action');
    if (isStringElement(action)) {
      const actionValue = action.toValue();
      if (actionValue === 'send' || actionValue === 'receive') {
        this.element.setMetaProperty('operation-action', actionValue);
      }
    }
    return result;
  }
}
export default OperationVisitor;