"use strict";

exports.__esModule = true;
exports.default = void 0;
var _ramdaAdjunct = require("ramda-adjunct");
var _index = require("../predicates/index.cjs");
const computeEdges = (element, edges = new WeakMap()) => {
  if ((0, _index.isMemberElement)(element)) {
    // @ts-ignore
    edges.set(element.key, element);
    // @ts-ignore
    computeEdges(element.key, edges);
    // @ts-ignore
    edges.set(element.value, element);
    // @ts-ignore
    computeEdges(element.value, edges);
  } else {
    element.children.forEach(childElement => {
      edges.set(childElement, element);
      computeEdges(childElement, edges);
    });
  }
  return edges;
};
const transcludeChildOfMemberElement = (search, replace, edges) => {
  const memberElement = edges.get(search);
  if (!(0, _index.isMemberElement)(memberElement)) {
    return;
  }
  if (memberElement.key === search) {
    memberElement.key = replace;
    edges.delete(search);
    edges.set(replace, memberElement);
  }
  if (memberElement.value === search) {
    memberElement.value = replace;
    edges.delete(search);
    edges.set(replace, memberElement);
  }
};
const transcludeChildOfObjectElement = (search, replace, edges) => {
  const objectElement = edges.get(search);
  if (!(0, _index.isObjectElement)(objectElement)) {
    return;
  }
  objectElement.content = objectElement.map((value, key, member) => {
    if (member === search) {
      edges.delete(search);
      edges.set(replace, objectElement);
      return replace;
    }
    return member;
  });
};
const transcludeChildOfArrayElement = (search, replace, edges) => {
  const arrayElement = edges.get(search);
  if (!(0, _index.isArrayElement)(arrayElement)) {
    return;
  }
  arrayElement.content = arrayElement.map(element => {
    if (element === search) {
      edges.delete(search);
      edges.set(replace, arrayElement);
      return replace;
    }
    return element;
  });
};

/**
 * This is a mutating stamp. If you don't want your Element to be mutated,
 * clone in before passing it to initializer of this stamp.
 * @public
 */

class Transcluder {
  element;
  edges;
  constructor({
    element
  }) {
    this.element = element;
  }
  transclude(search, replace) {
    var _this$edges;
    // shortcut 1. - replacing entire ApiDOM tree
    if (search === this.element) return replace;
    // shortcut 2. - replacing nothing
    if (search === replace) return this.element;
    this.edges = (_this$edges = this.edges) != null ? _this$edges : computeEdges(this.element);
    const parent = this.edges.get(search);
    if ((0, _ramdaAdjunct.isUndefined)(parent)) {
      return undefined;
    }

    /**
     * This predicate must be first because ObjectElement extends ArrayElement.
     * isArrayElement returns true for ObjectElements.
     * (classical problems with polymorphism)
     */
    if ((0, _index.isObjectElement)(parent)) {
      // @ts-ignore
      transcludeChildOfObjectElement(search, replace, this.edges);
    } else if ((0, _index.isArrayElement)(parent)) {
      transcludeChildOfArrayElement(search, replace, this.edges);
    } else if ((0, _index.isMemberElement)(parent)) {
      transcludeChildOfMemberElement(search, replace, this.edges);
    }
    return this.element;
  }
}
var _default = exports.default = Transcluder;