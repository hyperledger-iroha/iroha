import ResolveStrategy from "../ResolveStrategy.mjs";
import ReferenceSet from "../../../ReferenceSet.mjs";
import { merge as mergeOptions } from "../../../options/util.mjs";
import UnmatchedDereferenceStrategyError from "../../../errors/UnmatchedDereferenceStrategyError.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ApiDOMResolveStrategy extends ResolveStrategy {
  constructor(options) {
    super({
      ...(options !== null && options !== void 0 ? options : {}),
      name: 'apidom'
    });
  }
  canResolve(file, options) {
    const dereferenceStrategy = options.dereference.strategies.find(strategy => strategy.name === 'apidom');
    if (dereferenceStrategy === undefined) {
      return false;
    }
    return dereferenceStrategy.canDereference(file, options);
  }
  async resolve(file, options) {
    const dereferenceStrategy = options.dereference.strategies.find(strategy => strategy.name === 'apidom');
    if (dereferenceStrategy === undefined) {
      throw new UnmatchedDereferenceStrategyError('"apidom" dereference strategy is not available.');
    }
    const refSet = new ReferenceSet();
    const mergedOptions = mergeOptions(options, {
      resolve: {
        internal: false
      },
      dereference: {
        refSet
      }
    });
    await dereferenceStrategy.dereference(file, mergedOptions);
    return refSet;
  }
}
export default ApiDOMResolveStrategy;