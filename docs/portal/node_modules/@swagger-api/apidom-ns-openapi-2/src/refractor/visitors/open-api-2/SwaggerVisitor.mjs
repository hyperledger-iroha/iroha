import { Mixin } from 'ts-mixer';
import { BREAK, toValue } from '@swagger-api/apidom-core';
import SwaggerVersionElement from "../../../elements/SwaggerVersion.mjs";
import SpecificationVisitor from "../SpecificationVisitor.mjs";
import FallbackVisitor from "../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class SwaggerVisitor extends Mixin(SpecificationVisitor, FallbackVisitor) {
  StringElement(stringElement) {
    const swaggerVersionElement = new SwaggerVersionElement(toValue(stringElement));
    this.copyMetaAndAttributes(stringElement, swaggerVersionElement);
    this.element = swaggerVersionElement;
    return BREAK;
  }
}
export default SwaggerVisitor;