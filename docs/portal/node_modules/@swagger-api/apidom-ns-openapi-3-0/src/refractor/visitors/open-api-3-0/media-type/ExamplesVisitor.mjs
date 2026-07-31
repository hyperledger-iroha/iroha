import BaseExamplesVisitor from "../ExamplesVisitor.mjs";
import MediaTypeExamples from "../../../../elements/nces/MediaTypeExamples.mjs";
/**
 * @public
 */
class ExamplesVisitor extends BaseExamplesVisitor {
  constructor(options) {
    super(options);
    this.element = new MediaTypeExamples();
  }
}
export default ExamplesVisitor;