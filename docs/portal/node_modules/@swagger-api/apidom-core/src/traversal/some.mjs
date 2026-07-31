import { isNotUndefined } from 'ramda-adjunct';
import find from "./find.mjs";
/**
 * Tests whether at least one element passes the predicate.
 * @public
 */
const some = (predicate, element) => {
  return isNotUndefined(find(predicate, element));
};
export default some;