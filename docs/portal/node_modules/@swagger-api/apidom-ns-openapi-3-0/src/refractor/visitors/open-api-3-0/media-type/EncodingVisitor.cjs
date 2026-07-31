"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _ramda = require("ramda");
var _MediaTypeEncoding = _interopRequireDefault(require("../../../../elements/nces/MediaTypeEncoding.cjs"));
var _MapVisitor = _interopRequireDefault(require("../../generics/MapVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class EncodingVisitor extends (0, _tsMixer.Mixin)(_MapVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _MediaTypeEncoding.default();
    this.specPath = (0, _ramda.always)(['document', 'objects', 'Encoding']);
  }
}
var _default = exports.default = EncodingVisitor;