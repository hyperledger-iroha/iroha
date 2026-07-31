"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _ramda = require("ramda");
var _NatsChannelBinding = _interopRequireDefault(require("../../../../../../elements/bindings/nats/NatsChannelBinding.cjs"));
var _FixedFieldsVisitor = _interopRequireDefault(require("../../../../generics/FixedFieldsVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../../../FallbackVisitor.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class NatsChannelBindingVisitor extends (0, _tsMixer.Mixin)(_FixedFieldsVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _NatsChannelBinding.default();
    this.specPath = (0, _ramda.always)(['document', 'objects', 'bindings', 'nats', 'ChannelBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
var _default = exports.default = NatsChannelBindingVisitor;