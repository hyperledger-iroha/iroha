import { StringElement, BREAK, toValue } from '@swagger-api/apidom-core';
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
class VersionVisitor extends FallbackVisitor {
  StringElement(stringElement) {
    this.element = new StringElement(toValue(stringElement));
    this.copyMetaAndAttributes(stringElement, this.element);
    this.element.classes.push('api-version');
    this.element.classes.push('version');
    return BREAK;
  }
}
export default VersionVisitor;