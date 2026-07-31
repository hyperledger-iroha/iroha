import { pathOr } from 'ramda';
import { PredicateVisitor, BREAK, visit } from "./visitor.mjs";
/**
 * Find first element that satisfies the provided predicate.
 * @public
 */
const find = (predicate, element) => {
  const visitor = new PredicateVisitor({
    predicate,
    returnOnTrue: BREAK
  });
  visit(element, visitor);
  return pathOr(undefined, [0], visitor.result);
};
export default find;