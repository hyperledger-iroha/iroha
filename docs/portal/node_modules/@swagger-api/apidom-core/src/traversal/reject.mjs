import { complement } from 'ramda';
import filter from "./filter.mjs";
/**
 * Complement of filter.
 * @public
 */
const reject = (predicate, element) => {
  return filter(complement(predicate), element);
};
export default reject;