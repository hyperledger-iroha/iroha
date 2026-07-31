import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import { Amqp1ServerBindingElement } from '@swagger-api/apidom-ns-asyncapi-2';
import FallbackVisitor from "../../FallbackVisitor.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class Amqp1ServerBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new Amqp1ServerBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'amqp1', 'ServerBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default Amqp1ServerBindingVisitor;