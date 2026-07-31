import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
class VersionVisitor extends FallbackVisitor {
  StringElement(stringElement) {
    const result = super.enter(stringElement);
    this.element.classes.push('arazzo-version');
    this.element.classes.push('version');
    return result;
  }
}
export default VersionVisitor;