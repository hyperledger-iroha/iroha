import FallbackVisitor from "../FallbackVisitor.mjs";
/**
 * @public
 */
class EnumVisitor extends FallbackVisitor {
  ArrayElement(arrayElement) {
    const result = this.enter(arrayElement);
    this.element.classes.push('json-schema-enum');
    return result;
  }
}
export default EnumVisitor;