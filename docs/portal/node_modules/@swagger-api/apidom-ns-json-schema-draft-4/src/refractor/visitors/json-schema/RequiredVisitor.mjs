import FallbackVisitor from "../FallbackVisitor.mjs";
/**
 * @public
 */
class RequiredVisitor extends FallbackVisitor {
  ArrayElement(arrayElement) {
    const result = this.enter(arrayElement);
    this.element.classes.push('json-schema-required');
    return result;
  }
}
export default RequiredVisitor;