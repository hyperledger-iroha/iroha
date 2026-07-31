import { pathOr } from 'ramda';
import { isFunction, noop } from 'ramda-adjunct';
import { visit, PredicateVisitor } from "./visitor.mjs";
import { isElement } from "../predicates/index.mjs";
/**
 * @public
 */
/**
 * @public
 */
export class CallbackVisitor extends PredicateVisitor {
  callback;
  constructor({
    callback = noop,
    ...rest
  } = {}) {
    super({
      ...rest
    });
    this.callback = callback;
  }
  enter(element) {
    if (this.predicate(element)) {
      this.callback(element);
      return this.returnOnTrue;
    }
    return this.returnOnFalse;
  }
}

/**
 * Executes the callback on this element and all descendants.
 * @public
 */
const traverse = (options, element) => {
  let callback;
  let predicate;
  if (isFunction(options)) {
    callback = options;
    predicate = isElement;
  } else {
    callback = pathOr(noop, ['callback'], options);
    predicate = pathOr(isElement, ['predicate'], options);
  }
  const visitor = new CallbackVisitor({
    callback,
    predicate
  });

  // @ts-ignore
  visit(element, visitor);
};
export default traverse;