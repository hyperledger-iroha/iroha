"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _ramda = require("ramda");
var _SuccessAction = _interopRequireDefault(require("../../../../elements/SuccessAction.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
var _FixedFieldsVisitor = _interopRequireDefault(require("../../generics/FixedFieldsVisitor.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class SuccessActionVisitor extends (0, _tsMixer.Mixin)(_FixedFieldsVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _SuccessAction.default();
    this.specPath = (0, _ramda.always)(['document', 'objects', 'SuccessAction']);
    this.canSupportSpecificationExtensions = true;
  }
}
var _default = exports.default = SuccessActionVisitor;