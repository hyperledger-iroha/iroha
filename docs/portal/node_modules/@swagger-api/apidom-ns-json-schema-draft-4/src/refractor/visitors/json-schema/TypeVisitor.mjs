import FallbackVisitor from "../FallbackVisitor.mjs";
/**
 * @public
 */
class TypeVisitor extends FallbackVisitor {
  StringElement(stringElement) {
    const result = this.enter(stringElement);
    this.element.classes.push('json-schema-type');
    return result;
  }
  ArrayElement(arrayElement) {
    const result = this.enter(arrayElement);
    this.element.classes.push('json-schema-type');
    return result;
  }
}
export default TypeVisitor;