"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ramda = require("ramda");
var _ramdaAdjunct = require("ramda-adjunct");
var _apidomCore = require("@swagger-api/apidom-core");
var _Visitor = _interopRequireDefault(require("./Visitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("./FallbackVisitor.cjs"));
/**
 * This is a base class for every visitor that does
 * internal look-ups to retrieve other child visitors.
 * @public
 */

/**
 * @public
 */
class SpecificationVisitor extends _Visitor.default {
  specObj;
  passingOptionsNames = ['specObj'];
  constructor({
    specObj,
    ...rest
  }) {
    super({
      ...rest
    });
    this.specObj = specObj;
  }
  retrievePassingOptions() {
    return (0, _ramda.pick)(this.passingOptionsNames, this);
  }
  retrieveFixedFields(specPath) {
    const fixedFields = (0, _ramda.path)(['visitors', ...specPath, 'fixedFields'], this.specObj);
    if (typeof fixedFields === 'object' && fixedFields !== null) {
      return Object.keys(fixedFields);
    }
    return [];
  }
  retrieveVisitor(specPath) {
    if ((0, _ramda.pathSatisfies)(_ramdaAdjunct.isFunction, ['visitors', ...specPath], this.specObj)) {
      return (0, _ramda.path)(['visitors', ...specPath], this.specObj);
    }
    return (0, _ramda.path)(['visitors', ...specPath, '$visitor'], this.specObj);
  }
  retrieveVisitorInstance(specPath, options = {}) {
    const passingOpts = this.retrievePassingOptions();
    const VisitorClz = this.retrieveVisitor(specPath);
    const visitorOpts = {
      ...passingOpts,
      ...options
    };
    return new VisitorClz(visitorOpts);
  }
  toRefractedElement(specPath, element, options = {}) {
    /**
     * This is `Visitor shortcut`: mechanism for short-circuiting the traversal and replacing
     * it by basic node cloning.
     *
     * Visiting the element is equivalent to cloning it if the prototype of a visitor
     * is the same as the prototype of FallbackVisitor. If that's the case, we can avoid
     * bootstrapping the traversal cycle for fields that don't require any special visiting.
     */
    const visitor = this.retrieveVisitorInstance(specPath, options);
    if (visitor instanceof _FallbackVisitor.default && (visitor == null ? void 0 : visitor.constructor) === _FallbackVisitor.default) {
      return (0, _apidomCore.cloneDeep)(element);
    }
    (0, _apidomCore.visit)(element, visitor, options);
    return visitor.element;
  }
}
var _default = exports.default = SpecificationVisitor;