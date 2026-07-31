import { Mixin } from 'ts-mixer';
import { test } from 'ramda';
import PatternedFieldsVisitor from "../../generics/PatternedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import ServersElement from "../../../../elements/Servers.mjs";
import { isReferenceLikeElement } from "../../../predicates.mjs";
import { isReferenceElement } from "../../../../predicates.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ServersVisitor extends Mixin(PatternedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ServersElement();
    this.element.classes.push('servers');
    this.specPath = element => {
      return isReferenceLikeElement(element) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'Server'];
    };
    this.canSupportSpecificationExtensions = false;
    // @ts-ignore
    this.fieldPatternPredicate = test(/^[A-Za-z0-9_-]+$/);
  }
  ObjectElement(objectElement) {
    const result = PatternedFieldsVisitor.prototype.ObjectElement.call(this, objectElement);

    // @ts-ignore
    this.element.filter(isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'server');
    });
    return result;
  }
}
export default ServersVisitor;