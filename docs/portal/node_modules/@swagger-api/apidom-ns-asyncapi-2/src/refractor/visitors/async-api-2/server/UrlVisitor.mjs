import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
class UrlVisitor extends FallbackVisitor {
  StringElement(stringElement) {
    const result = super.enter(stringElement);
    this.element.classes.push('server-url');
    return result;
  }
}
export default UrlVisitor;