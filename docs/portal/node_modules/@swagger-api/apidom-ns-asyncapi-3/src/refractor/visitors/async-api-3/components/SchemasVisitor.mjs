import { Mixin } from 'ts-mixer';
import { isReferenceLikeElement } from '@swagger-api/apidom-ns-asyncapi-2';
import ComponentsSchemasElement from "../../../../elements/nces/ComponentsSchemas.mjs";
import MapVisitor from "../../generics/MapVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import { isMultiFormatSchemaElement, isReferenceElement } from "../../../../predicates.mjs";
import { isMultiFormatSchemaLikeElement } from "../../../predicates.mjs";
/**
 * @public
 */
/**
 * @public
 */
class SchemasVisitor extends Mixin(MapVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ComponentsSchemasElement();
    this.specPath = element => isReferenceLikeElement(element) ? ['document', 'objects', 'Reference'] : isMultiFormatSchemaLikeElement(element) ? ['document', 'objects', 'MultiFormatSchema'] : ['document', 'objects', 'Schema'];
  }
  ObjectElement(objectElement) {
    const result = MapVisitor.prototype.ObjectElement.call(this, objectElement);

    // @ts-ignore
    this.element.filter(isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'schema');
    });
    this.element.filter(isMultiFormatSchemaElement)
    // @ts-ignore
    .forEach(multiFormatSchemaElement => {
      multiFormatSchemaElement.setMetaProperty('multiformat-schema-element', 'schema');
    });
    return result;
  }
}
export default SchemasVisitor;