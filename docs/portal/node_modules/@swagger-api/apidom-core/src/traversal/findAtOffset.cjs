"use strict";

exports.__esModule = true;
exports.default = void 0;
var _ramda = require("ramda");
var _ramdaAdjunct = require("ramda-adjunct");
var _index = require("../predicates/index.cjs");
var _visitor = require("./visitor.cjs");
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
    if (!(0, _index.hasElementSourceMap)(element)) {
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
  if ((0, _ramdaAdjunct.isNumber)(options)) {
    offset = options;
    includeRightBound = false;
  } else {
    offset = (0, _ramda.pathOr)(0, ['offset'], options);
    includeRightBound = (0, _ramda.pathOr)(false, ['includeRightBound'], options);
  }
  const visitor = new Visitor({
    offset,
    includeRightBound
  });
  (0, _visitor.visit)(element, visitor);
  return (0, _ramda.last)(visitor.result);
};
var _default = exports.default = findAtOffset;