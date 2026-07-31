"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _Resolver = _interopRequireDefault(require("../Resolver.cjs"));
var _ResolverError = _interopRequireDefault(require("../../../errors/ResolverError.cjs"));
/**
 * @public
 */
class FileResolver extends _Resolver.default {
  constructor() {
    super({
      name: 'file'
    });
  }

  // eslint-disable-next-line class-methods-use-this
  canRead() {
    return false;
  }

  // eslint-disable-next-line class-methods-use-this
  read() {
    throw new _ResolverError.default('FileResolver is not intended to be used in browser context.');
  }
}
var _default = exports.default = FileResolver;