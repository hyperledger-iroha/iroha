"use strict";

exports.__esModule = true;
exports.visit = exports.keyMapDefault = exports.isNode = exports.getNodeType = exports.cloneNode = exports.PredicateVisitor = void 0;
var _ramda = require("ramda");
var _ramdaAdjunct = require("ramda-adjunct");
var _apidomAst = require("@swagger-api/apidom-ast");
exports.BREAK = _apidomAst.BREAK;
exports.mergeAllVisitors = _apidomAst.mergeAllVisitors;
var _index = require("../predicates/index.cjs");
var _index2 = require("../clone/index.cjs");
/**
 * @public
 */
const getNodeType = element => {
  /*
   * We're translating every possible higher element type to primitive minim type here.
   * We're using polymorphism to recognize any higher element type as ObjectElement or ArrayElement.
   * Using polymorphism allows us to assume any namespace.
   *
   * There is a problem with naming visitor methods described here: https://github.com/babel/babel/discussions/12874
   */
  return (0, _index.isObjectElement)(element) ? 'ObjectElement' : (0, _index.isArrayElement)(element) ? 'ArrayElement' : (0, _index.isMemberElement)(element) ? 'MemberElement' : (0, _index.isStringElement)(element) ? 'StringElement' : (0, _index.isBooleanElement)(element) ? 'BooleanElement' : (0, _index.isNumberElement)(element) ? 'NumberElement' : (0, _index.isNullElement)(element) ? 'NullElement' : (0, _index.isLinkElement)(element) ? 'LinkElement' : (0, _index.isRefElement)(element) ? 'RefElement' : undefined;
};

/**
 * @public
 */
exports.getNodeType = getNodeType;
const cloneNode = node => {
  if ((0, _index.isElement)(node)) {
    return (0, _index2.cloneShallow)(node);
  }
  return (0, _apidomAst.cloneNode)(node);
};

// isNode :: Node -> Boolean
exports.cloneNode = cloneNode;
const isNode = exports.isNode = (0, _ramda.pipe)(getNodeType, _ramdaAdjunct.isString);

/**
 * @public
 */
const keyMapDefault = exports.keyMapDefault = {
  ObjectElement: ['content'],
  ArrayElement: ['content'],
  MemberElement: ['key', 'value'],
  StringElement: [],
  BooleanElement: [],
  NumberElement: [],
  NullElement: [],
  RefElement: [],
  LinkElement: [],
  Annotation: [],
  Comment: [],
  ParseResultElement: ['content']
};
class PredicateVisitor {
  result;
  predicate;
  returnOnTrue;
  returnOnFalse;
  constructor({
    predicate = _ramda.F,
    returnOnTrue,
    returnOnFalse
  } = {}) {
    this.result = [];
    this.predicate = predicate;
    this.returnOnTrue = returnOnTrue;
    this.returnOnFalse = returnOnFalse;
  }
  enter(element) {
    if (this.predicate(element)) {
      this.result.push(element);
      return this.returnOnTrue;
    }
    return this.returnOnFalse;
  }
}

/**
 * @public
 */
exports.PredicateVisitor = PredicateVisitor;
const visit = (root,
// @ts-ignore
visitor, {
  keyMap = keyMapDefault,
  ...rest
} = {}) => {
  // @ts-ignore
  return (0, _apidomAst.visit)(root, visitor, {
    // @ts-ignore
    keyMap,
    // @ts-ignore
    nodeTypeGetter: getNodeType,
    nodePredicate: isNode,
    nodeCloneFn: cloneNode,
    ...rest
  });
};

// @ts-ignore
exports.visit = visit;
visit[Symbol.for('nodejs.util.promisify.custom')] = async (root,
// @ts-ignore
visitor, {
  keyMap = keyMapDefault,
  ...rest
} = {}) => {
  // @ts-ignore
  return _apidomAst.visit[Symbol.for('nodejs.util.promisify.custom')](root, visitor, {
    // @ts-ignore
    keyMap,
    // @ts-ignore
    nodeTypeGetter: getNodeType,
    nodePredicate: isNode,
    nodeCloneFn: cloneNode,
    ...rest
  });
};