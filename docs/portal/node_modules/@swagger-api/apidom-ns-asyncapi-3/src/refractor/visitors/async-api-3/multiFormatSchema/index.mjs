import { Mixin } from 'ts-mixer';
import { always, defaultTo } from 'ramda';
import { toValue, isObjectElement } from '@swagger-api/apidom-core';
import { isReferenceLikeElement } from '@swagger-api/apidom-ns-asyncapi-2';
import mediaTypes from "../../../../media-types.mjs";
import MultiFormatSchemaElement from "../../../../elements/MultiFormatSchema.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
/**
 * Implementation of refracting according `schemaFormat` fixed field is now limited,
 * and currently only supports `AsyncAPI Schema Object 3.0.0`
 * @public
 */
/**
 * @public
 */
class MultiFormatSchemaVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new MultiFormatSchemaElement();
    this.specPath = always(['document', 'objects', 'MultiFormatSchema']);
    this.canSupportSpecificationExtensions = true;
  }
  ObjectElement(objectElement) {
    const result = FixedFieldsVisitor.prototype.ObjectElement.call(this, objectElement);
    const schema = this.element.get('schema');
    const schemaFormat = defaultTo(mediaTypes.latest(), toValue(objectElement.get('schemaFormat')));
    if (mediaTypes.includes(schemaFormat) && isReferenceLikeElement(schema)) {
      // refract to ReferenceElement
      const referenceElement = this.toRefractedElement(['document', 'objects', 'Reference'], schema);
      referenceElement.meta.set('referenced-element', 'schema');
      this.element.schema = referenceElement;
    } else if (mediaTypes.includes(schemaFormat) && isObjectElement(this.element.schema)) {
      this.element.schema = this.toRefractedElement(['document', 'objects', 'Schema'], this.element.schema);
    }
    return result;
  }
}
export default MultiFormatSchemaVisitor;