"use strict";

var _interopRequireWildcard = require("@babel/runtime-corejs3/helpers/interopRequireWildcard").default;
var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.isStringElement = exports.isRefElement = exports.isPrimitiveElement = exports.isParseResultElement = exports.isObjectElement = exports.isNumberElement = exports.isNullElement = exports.isMemberElement = exports.isLinkElement = exports.isElement = exports.isCommentElement = exports.isBooleanElement = exports.isArrayElement = exports.isAnnotationElement = exports.includesSymbols = exports.includesClasses = exports.hasElementSourceMap = void 0;
var _minim = require("minim");
var _ramda = require("ramda");
var _ramdaAdjunct = require("ramda-adjunct");
var _Annotation = _interopRequireDefault(require("../elements/Annotation.cjs"));
var _Comment = _interopRequireDefault(require("../elements/Comment.cjs"));
var _ParseResult = _interopRequireDefault(require("../elements/ParseResult.cjs"));
var _helpers = _interopRequireWildcard(require("./helpers.cjs"));
/**
 * @public
 */
const isElement = exports.isElement = (0, _helpers.default)(({
  hasBasicElementProps,
  primitiveEq
}) => {
  return element => element instanceof _minim.Element || hasBasicElementProps(element) && primitiveEq(undefined, element);
});

/**
 * @public
 */
const isStringElement = exports.isStringElement = (0, _helpers.default)(({
  hasBasicElementProps,
  primitiveEq
}) => {
  return element => element instanceof _minim.StringElement || hasBasicElementProps(element) && primitiveEq('string', element);
});

/**
 * @public
 */
const isNumberElement = exports.isNumberElement = (0, _helpers.default)(({
  hasBasicElementProps,
  primitiveEq
}) => {
  return element => element instanceof _minim.NumberElement || hasBasicElementProps(element) && primitiveEq('number', element);
});

/**
 * @public
 */
const isNullElement = exports.isNullElement = (0, _helpers.default)(({
  hasBasicElementProps,
  primitiveEq
}) => {
  return element => element instanceof _minim.NullElement || hasBasicElementProps(element) && primitiveEq('null', element);
});

/**
 * @public
 */
const isBooleanElement = exports.isBooleanElement = (0, _helpers.default)(({
  hasBasicElementProps,
  primitiveEq
}) => {
  return element => element instanceof _minim.BooleanElement || hasBasicElementProps(element) && primitiveEq('boolean', element);
});

/**
 * @public
 */
const isObjectElement = exports.isObjectElement = (0, _helpers.default)(({
  hasBasicElementProps,
  primitiveEq,
  hasMethod
}) => {
  return element => element instanceof _minim.ObjectElement || hasBasicElementProps(element) && primitiveEq('object', element) && hasMethod('keys', element) && hasMethod('values', element) && hasMethod('items', element);
});

/**
 * @public
 */
const isArrayElement = exports.isArrayElement = (0, _helpers.default)(({
  hasBasicElementProps,
  primitiveEq,
  hasMethod
}) => {
  return element => element instanceof _minim.ArrayElement && !(element instanceof _minim.ObjectElement) || hasBasicElementProps(element) && primitiveEq('array', element) && hasMethod('push', element) && hasMethod('unshift', element) && hasMethod('map', element) && hasMethod('reduce', element);
});

/**
 * @public
 */
const isMemberElement = exports.isMemberElement = (0, _helpers.default)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _minim.MemberElement || hasBasicElementProps(element) && isElementType('member', element) && primitiveEq(undefined, element);
});

/**
 * @public
 */
const isLinkElement = exports.isLinkElement = (0, _helpers.default)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _minim.LinkElement || hasBasicElementProps(element) && isElementType('link', element) && primitiveEq(undefined, element);
});

/**
 * @public
 */
const isRefElement = exports.isRefElement = (0, _helpers.default)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _minim.RefElement || hasBasicElementProps(element) && isElementType('ref', element) && primitiveEq(undefined, element);
});

/**
 * @public
 */
const isAnnotationElement = exports.isAnnotationElement = (0, _helpers.default)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Annotation.default || hasBasicElementProps(element) && isElementType('annotation', element) && primitiveEq('array', element);
});

/**
 * @public
 */
const isCommentElement = exports.isCommentElement = (0, _helpers.default)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Comment.default || hasBasicElementProps(element) && isElementType('comment', element) && primitiveEq('string', element);
});

/**
 * @public
 */
const isParseResultElement = exports.isParseResultElement = (0, _helpers.default)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _ParseResult.default || hasBasicElementProps(element) && isElementType('parseResult', element) && primitiveEq('array', element);
});

/**
 * @public
 */

/**
 * @public
 */
const isPrimitiveElement = element => {
  return (0, _helpers.isElementType)('object', element) || (0, _helpers.isElementType)('array', element) || (0, _helpers.isElementType)('boolean', element) || (0, _helpers.isElementType)('number', element) || (0, _helpers.isElementType)('string', element) || (0, _helpers.isElementType)('null', element) || (0, _helpers.isElementType)('member', element);
};

/**
 * @public
 */
exports.isPrimitiveElement = isPrimitiveElement;
const hasElementSourceMap = element => {
  if (!isElement(element)) {
    return false;
  }
  return Number.isInteger(element.startPositionRow) && Number.isInteger(element.startPositionColumn) && Number.isInteger(element.startIndex) && Number.isInteger(element.endPositionRow) && Number.isInteger(element.endPositionColumn) && Number.isInteger(element.endIndex);
};

/**
 * @public
 */
exports.hasElementSourceMap = hasElementSourceMap;
const includesSymbols = (symbols, element) => {
  if (symbols.length === 0) {
    return true;
  }
  const elementSymbols = element.attributes.get('symbols');
  if (!isArrayElement(elementSymbols)) {
    return false;
  }
  return (0, _ramda.all)((0, _ramdaAdjunct.included)(elementSymbols.toValue()), symbols);
};

/**
 * @public
 */
exports.includesSymbols = includesSymbols;
const includesClasses = (classes, element) => {
  if (classes.length === 0) {
    return true;
  }
  return (0, _ramda.all)((0, _ramdaAdjunct.included)(element.classes.toValue()), classes);
};
exports.includesClasses = includesClasses;