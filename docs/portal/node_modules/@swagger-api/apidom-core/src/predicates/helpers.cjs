"use strict";

exports.__esModule = true;
exports.primitiveEq = exports.isElementType = exports.hasMethod = exports.hasClass = exports.hasBasicElementProps = exports.default = void 0;
var _minim = require("minim");
/**
 * @public
 */

/**
 * @public
 */

/**
 * @public
 */

/**
 * @public
 */

/**
 * @public
 */

/**
 * @public
 */

/**
 * @public
 */

/**
 * @public
 */
const hasMethod = (name, element) => {
  return typeof element === 'object' && element !== null && name in element && typeof element[name] === 'function';
};

/**
 * @public
 */
exports.hasMethod = hasMethod;
const hasBasicElementProps = element => typeof element === 'object' && element != null && '_storedElement' in element && typeof element._storedElement === 'string' &&
// eslint-disable-line no-underscore-dangle
'_content' in element;

/**
 * @public
 */
exports.hasBasicElementProps = hasBasicElementProps;
const primitiveEq = (val, element) => {
  if (typeof element === 'object' && element !== null && 'primitive' in element) {
    return typeof element.primitive === 'function' && element.primitive() === val;
  }
  return false;
};

/**
 * @public
 */
exports.primitiveEq = primitiveEq;
const hasClass = (cls, element) => {
  return typeof element === 'object' && element !== null && 'classes' in element && (Array.isArray(element.classes) || element.classes instanceof _minim.ArrayElement) && element.classes.includes(cls);
};

/**
 * @public
 */
exports.hasClass = hasClass;
const isElementType = (name, element) => typeof element === 'object' && element !== null && 'element' in element && element.element === name;

/**
 * @public
 */
exports.isElementType = isElementType;
const createPredicate = predicateCreator => {
  return predicateCreator({
    hasMethod,
    hasBasicElementProps,
    primitiveEq,
    isElementType,
    hasClass
  });
};
var _default = exports.default = createPredicate;