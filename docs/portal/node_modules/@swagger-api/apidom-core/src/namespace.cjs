"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.createNamespace = exports.Namespace = void 0;
var _minim = require("minim");
var _ramdaAdjunct = require("ramda-adjunct");
var _Annotation = _interopRequireDefault(require("./elements/Annotation.cjs"));
var _Comment = _interopRequireDefault(require("./elements/Comment.cjs"));
var _ParseResult = _interopRequireDefault(require("./elements/ParseResult.cjs"));
/**
 * @public
 */
class Namespace extends _minim.Namespace {
  constructor() {
    super();
    this.register('annotation', _Annotation.default);
    this.register('comment', _Comment.default);
    this.register('parseResult', _ParseResult.default);
  }
}

/**
 * @public
 */
exports.Namespace = Namespace;
const namespace = new Namespace();

/**
 * @public
 */
const createNamespace = namespacePlugin => {
  const namespaceInstance = new Namespace();
  if ((0, _ramdaAdjunct.isPlainObject)(namespacePlugin)) {
    namespaceInstance.use(namespacePlugin);
  }
  return namespaceInstance;
};
exports.createNamespace = createNamespace;
var _default = exports.default = namespace;