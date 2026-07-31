import { isNodeType } from "../../predicates.mjs";
/**
 * @public
 */
export const isDocument = node => isNodeType('document', node);

/**
 * @public
 */
export const isString = node => isNodeType('string', node);

/**
 * @public
 */
export const isFalse = node => isNodeType('false', node);

/**
 * @public
 */
export const isTrue = node => isNodeType('true', node);

/**
 * @public
 */
export const isNull = node => isNodeType('null', node);

/**
 * @public
 */
export const isNumber = node => isNodeType('number', node);

/**
 * @public
 */
export const isArray = node => isNodeType('array', node);

/**
 * @public
 */
export const isObject = node => isNodeType('object', node);

/**
 * @public
 */
export const isStringContent = node => isNodeType('stringContent', node);

/**
 * @public
 */
export const isEscapeSequence = node => isNodeType('escapeSequence', node);

/**
 * @public
 */
export const isProperty = node => isNodeType('property', node);

/**
 * @public
 */
export const isKey = node => isNodeType('key', node);