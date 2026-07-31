import CSTTranslator from "./CSTTranslator.mjs";
class XMLTranslator extends CSTTranslator {
  getTree() {
    return this.toXml();
  }
}
export default XMLTranslator;