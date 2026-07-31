"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
var _mediaTypes = _interopRequireDefault(require("../../../../media-types.cjs"));
var _Message = _interopRequireDefault(require("../../../../elements/Message.cjs"));
var _FixedFieldsVisitor = _interopRequireDefault(require("../../generics/FixedFieldsVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
var _predicates = require("../../../predicates.cjs");
/**
 * Implementation of refracting according `schemaFormat` fixed field is now limited,
 * and currently only supports `AsyncAPI Schema Object >= 2.0.0 <=2.6.0.`
 * @public
 */

/**
 * @public
 */
class MessageVisitor extends (0, _tsMixer.Mixin)(_FixedFieldsVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _Message.default();
    this.specPath = (0, _ramda.always)(['document', 'objects', 'Message']);
    this.canSupportSpecificationExtensions = true;
  }
  ObjectElement(objectElement) {
    const result = _FixedFieldsVisitor.default.prototype.ObjectElement.call(this, objectElement);
    const payload = this.element.get('payload');
    const schemaFormat = (0, _ramda.defaultTo)(_mediaTypes.default.latest(), (0, _apidomCore.toValue)(objectElement.get('schemaFormat')));
    if (_mediaTypes.default.includes(schemaFormat) && (0, _predicates.isReferenceLikeElement)(payload)) {
      // refract to ReferenceElement
      const referenceElement = this.toRefractedElement(['document', 'objects', 'Reference'], payload);
      referenceElement.meta.set('referenced-element', 'schema');
      this.element.payload = referenceElement;
    } else if (_mediaTypes.default.includes(schemaFormat) && (0, _apidomCore.isObjectElement)(this.element.payload)) {
      this.element.payload = this.toRefractedElement(['document', 'objects', 'Schema'], payload);
    }
    return result;
  }
}
var _default = exports.default = MessageVisitor;