"use strict";

exports.__esModule = true;
exports.default = void 0;
/**
 * @public
 */

/**
 * @public
 */
class Node {
  static type = 'node';
  type = 'node';
  isMissing;
  children;
  startPositionRow;
  startPositionColumn;
  startIndex;
  endPositionRow;
  endPositionColumn;
  endIndex;
  constructor({
    children = [],
    isMissing = false,
    startPositionRow,
    startPositionColumn,
    startIndex,
    endPositionRow,
    endPositionColumn,
    endIndex
  } = {}) {
    this.type = this.constructor.type;
    this.isMissing = isMissing;
    this.children = children;
    this.startPositionRow = startPositionRow;
    this.startPositionColumn = startPositionColumn;
    this.startIndex = startIndex;
    this.endPositionRow = endPositionRow;
    this.endPositionColumn = endPositionColumn;
    this.endIndex = endIndex;
  }

  // creates shallow clone of node
  clone() {
    // 1. copy has same prototype as orig
    const copy = Object.create(Object.getPrototypeOf(this));

    // 2. copy has all of orig’s properties
    Object.getOwnPropertyNames(this) // (1)
    .forEach(propKey => {
      // (2)
      const descriptor = Object.getOwnPropertyDescriptor(this, propKey); // (3)
      // @ts-ignore
      Object.defineProperty(copy, propKey, descriptor); // (4)
    });
    return copy;
  }
}
var _default = exports.default = Node;