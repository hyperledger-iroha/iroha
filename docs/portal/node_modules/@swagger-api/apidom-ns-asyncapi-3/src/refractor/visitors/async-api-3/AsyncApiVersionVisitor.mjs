import { Mixin } from 'ts-mixer';
import { BREAK, toValue } from '@swagger-api/apidom-core';
import FallbackVisitor from "../FallbackVisitor.mjs";
import SpecificationVisitor from "../SpecificationVisitor.mjs";
import AsyncApiVersionElement from "../../../elements/AsyncApiVersion.mjs";
/**
 * @public
 */
/**
 * @public
 */
class AsyncApiVersionVisitor extends Mixin(SpecificationVisitor, FallbackVisitor) {
  StringElement(stringElement) {
    const asyncApiVersionElement = new AsyncApiVersionElement(toValue(stringElement));
    this.copyMetaAndAttributes(stringElement, asyncApiVersionElement);
    this.element = asyncApiVersionElement;
    return BREAK;
  }
}
export default AsyncApiVersionVisitor;