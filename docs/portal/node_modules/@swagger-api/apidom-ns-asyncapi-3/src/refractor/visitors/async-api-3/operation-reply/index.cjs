"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _ramda = require("ramda");
var _OperationReply = _interopRequireDefault(require("../../../../elements/OperationReply.cjs"));
var _FixedFieldsVisitor = _interopRequireDefault(require("../../generics/FixedFieldsVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
// filepath: packages/apidom-ns-asyncapi-3/src/refractor/visitors/operation/OperationReplyVisitor.ts

/**
 * @public
 */
class OperationReplyVisitor extends (0, _tsMixer.Mixin)(_FixedFieldsVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _OperationReply.default();
    this.specPath = (0, _ramda.always)(['document', 'objects', 'OperationReply']);
    this.canSupportSpecificationExtensions = true;
  }
  ObjectElement(objectElement) {
    const result = _FixedFieldsVisitor.default.prototype.ObjectElement.call(this, objectElement);
    return result;
  }
}
var _default = exports.default = OperationReplyVisitor;