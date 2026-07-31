import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import { isStringElement } from '@swagger-api/apidom-core';
import ExampleElement from "../../../../elements/Example.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ExampleVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ExampleElement();
    this.specPath = always(['document', 'objects', 'Example']);
    this.canSupportSpecificationExtensions = true;
  }
  ObjectElement(objectElement) {
    const result = FixedFieldsVisitor.prototype.ObjectElement.call(this, objectElement);

    // mark this ExampleElement with reference metadata
    if (isStringElement(this.element.externalValue)) {
      this.element.classes.push('reference-element');
    }
    return result;
  }
}
export default ExampleVisitor;