import { last, pathOr } from 'ramda';
import { isNumber } from 'ramda-adjunct';
import { hasElementSourceMap } from "../predicates/index.mjs";
import { visit } from "./visitor.mjs";
class Visitor {
  result;
  offset;
  includeRightBound;
  constructor({
    offset = 0,
    includeRightBound = false
  } = {}) {
    this.result = [];
    this.offset = offset;
    this.includeRightBound = includeRightBound;
  }
  enter(element) {
    if (!hasElementSourceMap(element)) {
      return undefined; // dive in
    }
    const {
      startIndex,
      endIndex
    } = element;
    const isWithinOffsetRange = this.offset >= startIndex && (this.offset < endIndex || this.includeRightBound && this.offset <= endIndex);
    if (isWithinOffsetRange) {
      this.result.push(element);
      return undefined; // push to stack and dive in
    }
    return false; // skip entire sub-tree
  }
}

/**
 * @public
 */

/**
 * Finds the most inner node at the given offset.
 * If includeRightBound is set, also finds nodes that end at the given offset.
 * @public
 */
const findAtOffset = (options, element) => {
  let offset;
  let includeRightBound;
  if (isNumber(options)) {
    offset = options;
    includeRightBound = false;
  } else {
    offset = pathOr(0, ['offset'], options);
    includeRightBound = pathOr(false, ['includeRightBound'], options);
  }
  const visitor = new Visitor({
    offset,
    includeRightBound
  });
  visit(element, visitor);
  return last(visitor.result);
};
export default findAtOffset;