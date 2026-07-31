import { Mixin } from 'ts-mixer';
import { T as stubTrue } from 'ramda';
import { isReferenceElement, isReferenceLikeElement, isSchemaElement } from '@swagger-api/apidom-ns-asyncapi-2';
import AlternatingVisitor from "../../generics/AlternatingVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import { isMultiFormatSchemaElement } from "../../../../predicates.mjs";
import { isMultiFormatSchemaLikeElement } from "../../../predicates.mjs";
/**
 * @public
 */
/**
 * @public
 */
class HeadersVisitor extends Mixin(AlternatingVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.alternator = [{
      predicate: isReferenceLikeElement,
      specPath: ['document', 'objects', 'Reference']
    }, {
      predicate: isMultiFormatSchemaLikeElement,
      specPath: ['document', 'objects', 'MultiFormatSchema']
    }, {
      predicate: stubTrue,
      specPath: ['document', 'objects', 'Schema']
    }];
  }
  ObjectElement(objectElement) {
    const result = AlternatingVisitor.prototype.enter.call(this, objectElement);
    if (isReferenceElement(this.element)) {
      this.element.setMetaProperty('referenced-element', 'ref-header');
    }
    if (isSchemaElement(this.element)) {
      this.element.setMetaProperty('schema', 'header-schema');
    }
    if (isMultiFormatSchemaElement(this.element)) {
      this.element.setMetaProperty('schema', 'header-multiformat-schema');
    }
    return result;
  }
}
export default HeadersVisitor;