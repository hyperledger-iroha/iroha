import { FallbackVisitor } from '@swagger-api/apidom-ns-json-schema-draft-7';
/**
 * @public
 */
class DependentRequiredVisitor extends FallbackVisitor {
  ObjectElement(objectElement) {
    const result = super.enter(objectElement);
    this.element.classes.push('json-schema-dependentRequired');
    return result;
  }
}
export default DependentRequiredVisitor;