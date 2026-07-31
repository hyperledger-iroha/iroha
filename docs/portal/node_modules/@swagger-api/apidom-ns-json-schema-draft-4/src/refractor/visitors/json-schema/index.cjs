"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _ramda = require("ramda");
var _ramdaAdjunct = require("ramda-adjunct");
var _apidomCore = require("@swagger-api/apidom-core");
var _FixedFieldsVisitor = _interopRequireDefault(require("../generics/FixedFieldsVisitor.cjs"));
var _ParentSchemaAwareVisitor = _interopRequireDefault(require("./ParentSchemaAwareVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../FallbackVisitor.cjs"));
var _JSONSchema = _interopRequireDefault(require("../../../elements/JSONSchema.cjs"));
var _predicates = require("../../../predicates.cjs");
/**
 * @public
 */

/**
 * @public
 */
class JSONSchemaVisitor extends (0, _tsMixer.Mixin)(_FixedFieldsVisitor.default, _ParentSchemaAwareVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _JSONSchema.default();
    this.specPath = (0, _ramda.always)(['document', 'objects', 'JSONSchema']);
  }

  // eslint-disable-next-line class-methods-use-this
  get defaultDialectIdentifier() {
    return 'http://json-schema.org/draft-04/schema#';
  }
  ObjectElement(objectElement) {
    this.handleDialectIdentifier(objectElement);
    this.handleSchemaIdentifier(objectElement);

    // for further processing consider this JSONSchema Element as parent for all sub-schemas
    this.parent = this.element;
    return _FixedFieldsVisitor.default.prototype.ObjectElement.call(this, objectElement);
  }
  handleDialectIdentifier(objectElement) {
    // handle $schema keyword in embedded resources
    if ((0, _ramdaAdjunct.isUndefined)(this.parent) && !(0, _apidomCore.isStringElement)(objectElement.get('$schema'))) {
      // no parent available and no $schema is defined, set default $schema
      this.element.setMetaProperty('inheritedDialectIdentifier', this.defaultDialectIdentifier);
    } else if ((0, _predicates.isJSONSchemaElement)(this.parent) && !(0, _apidomCore.isStringElement)(objectElement.get('$schema'))) {
      // parent is available and no $schema is defined, set parent $schema
      const inheritedDialectIdentifier = (0, _ramda.defaultTo)((0, _apidomCore.toValue)(this.parent.meta.get('inheritedDialectIdentifier')), (0, _apidomCore.toValue)(this.parent.$schema));
      this.element.setMetaProperty('inheritedDialectIdentifier', inheritedDialectIdentifier);
    }
  }
  handleSchemaIdentifier(objectElement, identifierKeyword = 'id') {
    // handle schema identifier in embedded resources
    // fetch parent's ancestorsSchemaIdentifiers
    const ancestorsSchemaIdentifiers = this.parent !== undefined ? (0, _apidomCore.cloneDeep)(this.parent.getMetaProperty('ancestorsSchemaIdentifiers', [])) : new _apidomCore.ArrayElement();
    // get current schema identifier
    const schemaIdentifier = (0, _apidomCore.toValue)(objectElement.get(identifierKeyword));

    // remember schema identifier if it's a non-empty strings
    if ((0, _ramdaAdjunct.isNonEmptyString)(schemaIdentifier)) {
      ancestorsSchemaIdentifiers.push(schemaIdentifier);
    }
    this.element.setMetaProperty('ancestorsSchemaIdentifiers', ancestorsSchemaIdentifiers);
  }
}
var _default = exports.default = JSONSchemaVisitor;