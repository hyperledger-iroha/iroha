"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
var _JSONReference = _interopRequireDefault(require("../../../../elements/JSONReference.cjs"));
var _FixedFieldsVisitor = _interopRequireDefault(require("../../generics/FixedFieldsVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class JSONReferenceVisitor extends (0, _tsMixer.Mixin)(_FixedFieldsVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _JSONReference.default();
    this.specPath = (0, _ramda.always)(['document', 'objects', 'JSONReference']);
  }
  ObjectElement(objectElement) {
    const result = _FixedFieldsVisitor.default.prototype.ObjectElement.call(this, objectElement);

    // mark this JSONReferenceElement with reference metadata
    if ((0, _apidomCore.isStringElement)(this.element.$ref)) {
      this.element.classes.push('reference-element');
    }
    return result;
  }
}
var _default = exports.default = JSONReferenceVisitor;