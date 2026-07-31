import { pathSatisfies, path, pick } from 'ramda';
import { isFunction } from 'ramda-adjunct';
import { visit, cloneDeep } from '@swagger-api/apidom-core';
import Visitor from "./Visitor.mjs";
import FallbackVisitor from "./FallbackVisitor.mjs";
/**
 * This is a base Type for every visitor that does
 * internal look-ups to retrieve other child visitors.
 * @public
 */
/**
 * @public
 */
class SpecificationVisitor extends Visitor {
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
    return pick(this.passingOptionsNames, this);
  }
  retrieveFixedFields(specPath) {
    const fixedFields = path(['visitors', ...specPath, 'fixedFields'], this.specObj);
    if (typeof fixedFields === 'object' && fixedFields !== null) {
      return Object.keys(fixedFields);
    }
    return [];
  }
  retrieveVisitor(specPath) {
    if (pathSatisfies(isFunction, ['visitors', ...specPath], this.specObj)) {
      return path(['visitors', ...specPath], this.specObj);
    }
    return path(['visitors', ...specPath, '$visitor'], this.specObj);
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
    if (visitor instanceof FallbackVisitor && (visitor === null || visitor === void 0 ? void 0 : visitor.constructor) === FallbackVisitor) {
      return cloneDeep(element);
    }
    visit(element, visitor, options);
    return visitor.element;
  }
}
export default SpecificationVisitor;