import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import { toValue } from '@swagger-api/apidom-core';
import CallbackElement from "../../../../elements/Callback.mjs";
import PatternedFieldsVisitor from "../../generics/PatternedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import MapVisitor from "../../generics/MapVisitor.mjs";
import { isPathItemElement } from "../../../../predicates.mjs";
/**
 * @public
 */
/**
 * @public
 */
class CallbackVisitor extends Mixin(PatternedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new CallbackElement();
    this.specPath = always(['document', 'objects', 'PathItem']);
    this.canSupportSpecificationExtensions = true;
    this.fieldPatternPredicate = value => /{(?<expression>[^}]{1,2083})}/.test(String(value)); // 2,083 characters is the maximum length of a URL in Chrome
  }
  ObjectElement(objectElement) {
    const result = MapVisitor.prototype.ObjectElement.call(this, objectElement);

    // decorate every PathItemElement with Callback Object expression metadata
    this.element.filter(isPathItemElement)
    // @ts-ignore
    .forEach((pathItemElement, key) => {
      pathItemElement.setMetaProperty('runtime-expression', toValue(key));
    });
    return result;
  }
}
export default CallbackVisitor;