import { ArraySlice } from 'minim';
import { PredicateVisitor, visit } from "./visitor.mjs";
/**
 * Finds all elements matching the predicate.
 * @public
 */
const filter = (predicate, element) => {
  const visitor = new PredicateVisitor({
    predicate
  });
  visit(element, visitor);
  return new ArraySlice(visitor.result);
};
export default filter;