import { Mixin } from 'ts-mixer';
import { BREAK, toValue } from '@swagger-api/apidom-core';
import SpecificationVisitor from "../SpecificationVisitor.mjs";
import FallbackVisitor from "../FallbackVisitor.mjs";
import OpenapiElement from "../../../elements/Openapi.mjs";
/**
 * @public
 */
/**
 * @public
 */
class OpenapiVisitor extends Mixin(SpecificationVisitor, FallbackVisitor) {
  StringElement(stringElement) {
    const openapiElement = new OpenapiElement(toValue(stringElement));
    this.copyMetaAndAttributes(stringElement, openapiElement);
    this.element = openapiElement;
    return BREAK;
  }
}
export default OpenapiVisitor;