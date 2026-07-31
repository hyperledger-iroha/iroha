"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _mediaTypes = _interopRequireDefault(require("../../../../media-types.cjs"));
var _MultiFormatSchema = _interopRequireDefault(require("../../../../elements/MultiFormatSchema.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
var _FixedFieldsVisitor = _interopRequireDefault(require("../../generics/FixedFieldsVisitor.cjs"));
/**
 * Implementation of refracting according `schemaFormat` fixed field is now limited,
 * and currently only supports `AsyncAPI Schema Object 3.0.0`
 * @public
 */

/**
 * @public
 */
class MultiFormatSchemaVisitor extends (0, _tsMixer.Mixin)(_FixedFieldsVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _MultiFormatSchema.default();
    this.specPath = (0, _ramda.always)(['document', 'objects', 'MultiFormatSchema']);
    this.canSupportSpecificationExtensions = true;
  }
  ObjectElement(objectElement) {
    const result = _FixedFieldsVisitor.default.prototype.ObjectElement.call(this, objectElement);
    const schema = this.element.get('schema');
    const schemaFormat = (0, _ramda.defaultTo)(_mediaTypes.default.latest(), (0, _apidomCore.toValue)(objectElement.get('schemaFormat')));
    if (_mediaTypes.default.includes(schemaFormat) && (0, _apidomNsAsyncapi.isReferenceLikeElement)(schema)) {
      // refract to ReferenceElement
      const referenceElement = this.toRefractedElement(['document', 'objects', 'Reference'], schema);
      referenceElement.meta.set('referenced-element', 'schema');
      this.element.schema = referenceElement;
    } else if (_mediaTypes.default.includes(schemaFormat) && (0, _apidomCore.isObjectElement)(this.element.schema)) {
      this.element.schema = this.toRefractedElement(['document', 'objects', 'Schema'], this.element.schema);
    }
    return result;
  }
}
var _default = exports.default = MultiFormatSchemaVisitor;