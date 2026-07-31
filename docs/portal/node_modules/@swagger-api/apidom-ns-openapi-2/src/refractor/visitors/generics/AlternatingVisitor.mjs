import { ifElse, always } from 'ramda';
import { dispatch, stubUndefined } from 'ramda-adjunct';
import { BREAK } from '@swagger-api/apidom-core';
import SpecificationVisitor from "../SpecificationVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
/**
 * @public
 */
class AlternatingVisitor extends SpecificationVisitor {
  alternator;
  constructor({
    alternator,
    ...rest
  }) {
    super({
      ...rest
    });
    this.alternator = alternator || [];
  }
  enter(element) {
    const functions = this.alternator.map(({
      predicate,
      specPath
    }) => ifElse(predicate, always(specPath), stubUndefined));
    const specPath = dispatch(functions)(element);
    this.element = this.toRefractedElement(specPath, element);
    return BREAK;
  }
}
export default AlternatingVisitor;