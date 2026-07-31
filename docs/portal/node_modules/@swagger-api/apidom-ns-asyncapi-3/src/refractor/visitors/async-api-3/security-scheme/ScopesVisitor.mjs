import { Mixin } from 'ts-mixer';
import { isStringElement, BREAK, cloneDeep } from '@swagger-api/apidom-core';
import SecuritySchemeScopes from "../../../../elements/nces/SecuritySchemeScopes.mjs";
import SpecificationVisitor from "../../SpecificationVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ScopesVisitor extends Mixin(SpecificationVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new SecuritySchemeScopes();
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      const element = cloneDeep(item);
      if (isStringElement(element)) {
        element.classes.push('scope-name');
      }
      this.element.push(element);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return BREAK;
  }
}
export default ScopesVisitor;