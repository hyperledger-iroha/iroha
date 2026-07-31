import { F as stubFalse, pipe } from 'ramda';
import { isString } from 'ramda-adjunct';
import { visit as astVisit, BREAK, mergeAllVisitors, cloneNode as cloneNodeDefault } from '@swagger-api/apidom-ast';
import { isElement, isMemberElement, isArrayElement, isStringElement, isBooleanElement, isLinkElement, isRefElement, isObjectElement, isNullElement, isNumberElement } from "../predicates/index.mjs";
import { cloneShallow } from "../clone/index.mjs";
export { BREAK, mergeAllVisitors };

/**
 * @public
 */
export const getNodeType = element => {
  /*
   * We're translating every possible higher element type to primitive minim type here.
   * We're using polymorphism to recognize any higher element type as ObjectElement or ArrayElement.
   * Using polymorphism allows us to assume any namespace.
   *
   * There is a problem with naming visitor methods described here: https://github.com/babel/babel/discussions/12874
   */
  return isObjectElement(element) ? 'ObjectElement' : isArrayElement(element) ? 'ArrayElement' : isMemberElement(element) ? 'MemberElement' : isStringElement(element) ? 'StringElement' : isBooleanElement(element) ? 'BooleanElement' : isNumberElement(element) ? 'NumberElement' : isNullElement(element) ? 'NullElement' : isLinkElement(element) ? 'LinkElement' : isRefElement(element) ? 'RefElement' : undefined;
};

/**
 * @public
 */
export const cloneNode = node => {
  if (isElement(node)) {
    return cloneShallow(node);
  }
  return cloneNodeDefault(node);
};

// isNode :: Node -> Boolean
export const isNode = pipe(getNodeType, isString);

/**
 * @public
 */
export const keyMapDefault = {
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
export class PredicateVisitor {
  result;
  predicate;
  returnOnTrue;
  returnOnFalse;
  constructor({
    predicate = stubFalse,
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
export const visit = (root,
// @ts-ignore
visitor, {
  keyMap = keyMapDefault,
  ...rest
} = {}) => {
  // @ts-ignore
  return astVisit(root, visitor, {
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
visit[Symbol.for('nodejs.util.promisify.custom')] = async (root,
// @ts-ignore
visitor, {
  keyMap = keyMapDefault,
  ...rest
} = {}) => {
  // @ts-ignore
  return astVisit[Symbol.for('nodejs.util.promisify.custom')](root, visitor, {
    // @ts-ignore
    keyMap,
    // @ts-ignore
    nodeTypeGetter: getNodeType,
    nodePredicate: isNode,
    nodeCloneFn: cloneNode,
    ...rest
  });
};