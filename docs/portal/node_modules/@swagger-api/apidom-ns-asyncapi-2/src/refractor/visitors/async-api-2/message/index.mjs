import { Mixin } from 'ts-mixer';
import { always, defaultTo } from 'ramda';
import { isObjectElement, toValue } from '@swagger-api/apidom-core';
import mediaTypes from "../../../../media-types.mjs";
import MessageElement from "../../../../elements/Message.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import { isReferenceLikeElement } from "../../../predicates.mjs";
/**
 * Implementation of refracting according `schemaFormat` fixed field is now limited,
 * and currently only supports `AsyncAPI Schema Object >= 2.0.0 <=2.6.0.`
 * @public
 */
/**
 * @public
 */
class MessageVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new MessageElement();
    this.specPath = always(['document', 'objects', 'Message']);
    this.canSupportSpecificationExtensions = true;
  }
  ObjectElement(objectElement) {
    const result = FixedFieldsVisitor.prototype.ObjectElement.call(this, objectElement);
    const payload = this.element.get('payload');
    const schemaFormat = defaultTo(mediaTypes.latest(), toValue(objectElement.get('schemaFormat')));
    if (mediaTypes.includes(schemaFormat) && isReferenceLikeElement(payload)) {
      // refract to ReferenceElement
      const referenceElement = this.toRefractedElement(['document', 'objects', 'Reference'], payload);
      referenceElement.meta.set('referenced-element', 'schema');
      this.element.payload = referenceElement;
    } else if (mediaTypes.includes(schemaFormat) && isObjectElement(this.element.payload)) {
      this.element.payload = this.toRefractedElement(['document', 'objects', 'Schema'], payload);
    }
    return result;
  }
}
export default MessageVisitor;