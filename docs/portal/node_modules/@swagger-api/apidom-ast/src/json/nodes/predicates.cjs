"use strict";

exports.__esModule = true;
exports.isTrue = exports.isStringContent = exports.isString = exports.isProperty = exports.isObject = exports.isNumber = exports.isNull = exports.isKey = exports.isFalse = exports.isEscapeSequence = exports.isDocument = exports.isArray = void 0;
var _predicates = require("../../predicates.cjs");
/**
 * @public
 */
const isDocument = node => (0, _predicates.isNodeType)('document', node);

/**
 * @public
 */
exports.isDocument = isDocument;
const isString = node => (0, _predicates.isNodeType)('string', node);

/**
 * @public
 */
exports.isString = isString;
const isFalse = node => (0, _predicates.isNodeType)('false', node);

/**
 * @public
 */
exports.isFalse = isFalse;
const isTrue = node => (0, _predicates.isNodeType)('true', node);

/**
 * @public
 */
exports.isTrue = isTrue;
const isNull = node => (0, _predicates.isNodeType)('null', node);

/**
 * @public
 */
exports.isNull = isNull;
const isNumber = node => (0, _predicates.isNodeType)('number', node);

/**
 * @public
 */
exports.isNumber = isNumber;
const isArray = node => (0, _predicates.isNodeType)('array', node);

/**
 * @public
 */
exports.isArray = isArray;
const isObject = node => (0, _predicates.isNodeType)('object', node);

/**
 * @public
 */
exports.isObject = isObject;
const isStringContent = node => (0, _predicates.isNodeType)('stringContent', node);

/**
 * @public
 */
exports.isStringContent = isStringContent;
const isEscapeSequence = node => (0, _predicates.isNodeType)('escapeSequence', node);

/**
 * @public
 */
exports.isEscapeSequence = isEscapeSequence;
const isProperty = node => (0, _predicates.isNodeType)('property', node);

/**
 * @public
 */
exports.isProperty = isProperty;
const isKey = node => (0, _predicates.isNodeType)('key', node);
exports.isKey = isKey;