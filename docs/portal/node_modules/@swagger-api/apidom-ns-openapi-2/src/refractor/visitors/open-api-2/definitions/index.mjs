import { Mixin } from 'ts-mixer';
import { isJSONReferenceLikeElement, isJSONReferenceElement } from '@swagger-api/apidom-ns-json-schema-draft-4';
import DefinitionsElement from "../../../../elements/Definitions.mjs";
import MapVisitor from "../../generics/MapVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class DefinitionsVisitor extends Mixin(MapVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new DefinitionsElement();
    this.specPath = element => {
      return isJSONReferenceLikeElement(element) ? ['document', 'objects', 'JSONReference'] : ['document', 'objects', 'Schema'];
    };
  }
  ObjectElement(objectElement) {
    const result = MapVisitor.prototype.ObjectElement.call(this, objectElement);

    // decorate every JSONReferenceElement with metadata about their referencing type
    this.element.filter(isJSONReferenceElement)
    // @ts-ignore
    .forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'schema');
    });
    return result;
  }
}
export default DefinitionsVisitor;