import CSTTranslator from "./CSTTranslator.mjs";
import unescape from "../../unescape.mjs";
class ASTTranslator extends CSTTranslator {
  getTree() {
    const {
      root
    } = super.getTree();
    return root.children.filter(({
      type
    }) => type === 'reference-token').map(({
      text
    }) => unescape(text));
  }
}
export default ASTTranslator;