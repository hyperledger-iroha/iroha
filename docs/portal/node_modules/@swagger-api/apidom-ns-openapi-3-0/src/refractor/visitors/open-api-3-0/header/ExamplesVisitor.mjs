import BaseExamplesVisitor from "../ExamplesVisitor.mjs";
import HeaderExamplesElement from "../../../../elements/nces/HeaderExamples.mjs";
/**
 * @public
 */
class ExamplesVisitor extends BaseExamplesVisitor {
  constructor(options) {
    super(options);
    this.element = new HeaderExamplesElement();
  }
}
export default ExamplesVisitor;