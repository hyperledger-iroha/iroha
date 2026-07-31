import { Element, StringElement, NumberElement, NullElement, BooleanElement, ArrayElement, ObjectElement, MemberElement, LinkElement, RefElement } from 'minim';
import { all } from 'ramda';
import { included } from 'ramda-adjunct';
import AnnotationElement from "../elements/Annotation.mjs";
import CommentElement from "../elements/Comment.mjs";
import ParserResultElement from "../elements/ParseResult.mjs";
import createPredicate, { isElementType as isElementTypeHelper } from "./helpers.mjs";
/**
 * @public
 */
export const isElement = createPredicate(({
  hasBasicElementProps,
  primitiveEq
}) => {
  return element => element instanceof Element || hasBasicElementProps(element) && primitiveEq(undefined, element);
});

/**
 * @public
 */
export const isStringElement = createPredicate(({
  hasBasicElementProps,
  primitiveEq
}) => {
  return element => element instanceof StringElement || hasBasicElementProps(element) && primitiveEq('string', element);
});

/**
 * @public
 */
export const isNumberElement = createPredicate(({
  hasBasicElementProps,
  primitiveEq
}) => {
  return element => element instanceof NumberElement || hasBasicElementProps(element) && primitiveEq('number', element);
});

/**
 * @public
 */
export const isNullElement = createPredicate(({
  hasBasicElementProps,
  primitiveEq
}) => {
  return element => element instanceof NullElement || hasBasicElementProps(element) && primitiveEq('null', element);
});

/**
 * @public
 */
export const isBooleanElement = createPredicate(({
  hasBasicElementProps,
  primitiveEq
}) => {
  return element => element instanceof BooleanElement || hasBasicElementProps(element) && primitiveEq('boolean', element);
});

/**
 * @public
 */
export const isObjectElement = createPredicate(({
  hasBasicElementProps,
  primitiveEq,
  hasMethod
}) => {
  return element => element instanceof ObjectElement || hasBasicElementProps(element) && primitiveEq('object', element) && hasMethod('keys', element) && hasMethod('values', element) && hasMethod('items', element);
});

/**
 * @public
 */
export const isArrayElement = createPredicate(({
  hasBasicElementProps,
  primitiveEq,
  hasMethod
}) => {
  return element => element instanceof ArrayElement && !(element instanceof ObjectElement) || hasBasicElementProps(element) && primitiveEq('array', element) && hasMethod('push', element) && hasMethod('unshift', element) && hasMethod('map', element) && hasMethod('reduce', element);
});

/**
 * @public
 */
export const isMemberElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof MemberElement || hasBasicElementProps(element) && isElementType('member', element) && primitiveEq(undefined, element);
});

/**
 * @public
 */
export const isLinkElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof LinkElement || hasBasicElementProps(element) && isElementType('link', element) && primitiveEq(undefined, element);
});

/**
 * @public
 */
export const isRefElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof RefElement || hasBasicElementProps(element) && isElementType('ref', element) && primitiveEq(undefined, element);
});

/**
 * @public
 */
export const isAnnotationElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof AnnotationElement || hasBasicElementProps(element) && isElementType('annotation', element) && primitiveEq('array', element);
});

/**
 * @public
 */
export const isCommentElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof CommentElement || hasBasicElementProps(element) && isElementType('comment', element) && primitiveEq('string', element);
});

/**
 * @public
 */
export const isParseResultElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof ParserResultElement || hasBasicElementProps(element) && isElementType('parseResult', element) && primitiveEq('array', element);
});

/**
 * @public
 */

/**
 * @public
 */
export const isPrimitiveElement = element => {
  return isElementTypeHelper('object', element) || isElementTypeHelper('array', element) || isElementTypeHelper('boolean', element) || isElementTypeHelper('number', element) || isElementTypeHelper('string', element) || isElementTypeHelper('null', element) || isElementTypeHelper('member', element);
};

/**
 * @public
 */
export const hasElementSourceMap = element => {
  if (!isElement(element)) {
    return false;
  }
  return Number.isInteger(element.startPositionRow) && Number.isInteger(element.startPositionColumn) && Number.isInteger(element.startIndex) && Number.isInteger(element.endPositionRow) && Number.isInteger(element.endPositionColumn) && Number.isInteger(element.endIndex);
};

/**
 * @public
 */
export const includesSymbols = (symbols, element) => {
  if (symbols.length === 0) {
    return true;
  }
  const elementSymbols = element.attributes.get('symbols');
  if (!isArrayElement(elementSymbols)) {
    return false;
  }
  return all(included(elementSymbols.toValue()), symbols);
};

/**
 * @public
 */
export const includesClasses = (classes, element) => {
  if (classes.length === 0) {
    return true;
  }
  return all(included(element.classes.toValue()), classes);
};