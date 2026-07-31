import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
class OperationIdVisitor extends FallbackVisitor {
  StringElement(stringElement) {
    const result = super.enter(stringElement);
    this.element.classes.push('reference-value');
    return result;
  }
}
export default OperationIdVisitor;