import { FallbackVisitor } from '@swagger-api/apidom-ns-json-schema-draft-7';
/**
 * @public
 */
class $refVisitor extends FallbackVisitor {
  StringElement(stringElement) {
    const result = super.enter(stringElement);
    this.element.classes.push('reference-value');
    return result;
  }
}
export default $refVisitor;