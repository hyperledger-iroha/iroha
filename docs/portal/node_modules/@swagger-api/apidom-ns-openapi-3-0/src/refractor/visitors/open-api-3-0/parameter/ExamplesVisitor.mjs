import BaseExamplesVisitor from "../ExamplesVisitor.mjs";
import ParameterExamplesElement from "../../../../elements/nces/ParameterExamples.mjs";
/**
 * @public
 */
class ExamplesVisitor extends BaseExamplesVisitor {
  constructor(options) {
    super(options);
    this.element = new ParameterExamplesElement();
  }
}
export default ExamplesVisitor;