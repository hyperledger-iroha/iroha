"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ResolveStrategy = _interopRequireDefault(require("../ResolveStrategy.cjs"));
var _ReferenceSet = _interopRequireDefault(require("../../../ReferenceSet.cjs"));
var _util = require("../../../options/util.cjs");
var _UnmatchedDereferenceStrategyError = _interopRequireDefault(require("../../../errors/UnmatchedDereferenceStrategyError.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class ApiDOMResolveStrategy extends _ResolveStrategy.default {
  constructor(options) {
    super({
      ...(options != null ? options : {}),
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
      throw new _UnmatchedDereferenceStrategyError.default('"apidom" dereference strategy is not available.');
    }
    const refSet = new _ReferenceSet.default();
    const mergedOptions = (0, _util.merge)(options, {
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
var _default = exports.default = ApiDOMResolveStrategy;