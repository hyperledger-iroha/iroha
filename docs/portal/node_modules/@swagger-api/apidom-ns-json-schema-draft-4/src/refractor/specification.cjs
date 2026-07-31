"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _FallbackVisitor = _interopRequireDefault(require("./visitors/FallbackVisitor.cjs"));
var _index = _interopRequireDefault(require("./visitors/json-schema/index.cjs"));
var _ItemsVisitor = _interopRequireDefault(require("./visitors/json-schema/ItemsVisitor.cjs"));
var _RequiredVisitor = _interopRequireDefault(require("./visitors/json-schema/RequiredVisitor.cjs"));
var _PropertiesVisitor = _interopRequireDefault(require("./visitors/json-schema/PropertiesVisitor.cjs"));
var _PatternPropertiesVisitor = _interopRequireDefault(require("./visitors/json-schema/PatternPropertiesVisitor.cjs"));
var _DependenciesVisitor = _interopRequireDefault(require("./visitors/json-schema/DependenciesVisitor.cjs"));
var _EnumVisitor = _interopRequireDefault(require("./visitors/json-schema/EnumVisitor.cjs"));
var _TypeVisitor = _interopRequireDefault(require("./visitors/json-schema/TypeVisitor.cjs"));
var _AllOfVisitor = _interopRequireDefault(require("./visitors/json-schema/AllOfVisitor.cjs"));
var _AnyOfVisitor = _interopRequireDefault(require("./visitors/json-schema/AnyOfVisitor.cjs"));
var _OneOfVisitor = _interopRequireDefault(require("./visitors/json-schema/OneOfVisitor.cjs"));
var _DefinitionsVisitor = _interopRequireDefault(require("./visitors/json-schema/DefinitionsVisitor.cjs"));
var _LinksVisitor = _interopRequireDefault(require("./visitors/json-schema/LinksVisitor.cjs"));
var _index2 = _interopRequireDefault(require("./visitors/json-schema/json-reference/index.cjs"));
var _$RefVisitor = _interopRequireDefault(require("./visitors/json-schema/json-reference/$RefVisitor.cjs"));
var _JSONSchemaOrJSONReferenceVisitor = _interopRequireDefault(require("./visitors/json-schema/JSONSchemaOrJSONReferenceVisitor.cjs"));
var _index3 = _interopRequireDefault(require("./visitors/json-schema/media/index.cjs"));
var _index4 = _interopRequireDefault(require("./visitors/json-schema/link-description/index.cjs"));
/**
 * Specification object allows us to have complete control over visitors
 * when traversing the ApiDOM.
 * Specification also allows us to create amended refractors from
 * existing ones by manipulating it.
 *
 * Note: Specification object allows to use absolute internal JSON pointers.
 * @public
 */
const specification = {
  visitors: {
    value: _FallbackVisitor.default,
    JSONSchemaOrJSONReferenceVisitor: _JSONSchemaOrJSONReferenceVisitor.default,
    document: {
      objects: {
        JSONSchema: {
          $visitor: _index.default,
          fixedFields: {
            // core vocabulary
            id: {
              $ref: '#/visitors/value'
            },
            $schema: {
              $ref: '#/visitors/value'
            },
            // validation vocabulary
            // validation keywords for numeric instances (number and integer)
            multipleOf: {
              $ref: '#/visitors/value'
            },
            maximum: {
              $ref: '#/visitors/value'
            },
            exclusiveMaximum: {
              $ref: '#/visitors/value'
            },
            minimum: {
              $ref: '#/visitors/value'
            },
            exclusiveMinimum: {
              $ref: '#/visitors/value'
            },
            // validation keywords for strings
            maxLength: {
              $ref: '#/visitors/value'
            },
            minLength: {
              $ref: '#/visitors/value'
            },
            pattern: {
              $ref: '#/visitors/value'
            },
            // validation keywords for arrays
            additionalItems: _JSONSchemaOrJSONReferenceVisitor.default,
            items: _ItemsVisitor.default,
            maxItems: {
              $ref: '#/visitors/value'
            },
            minItems: {
              $ref: '#/visitors/value'
            },
            uniqueItems: {
              $ref: '#/visitors/value'
            },
            // validation keywords for objects
            maxProperties: {
              $ref: '#/visitors/value'
            },
            minProperties: {
              $ref: '#/visitors/value'
            },
            required: _RequiredVisitor.default,
            properties: _PropertiesVisitor.default,
            additionalProperties: _JSONSchemaOrJSONReferenceVisitor.default,
            patternProperties: _PatternPropertiesVisitor.default,
            dependencies: _DependenciesVisitor.default,
            // validation keywords for any instance type
            enum: _EnumVisitor.default,
            type: _TypeVisitor.default,
            allOf: _AllOfVisitor.default,
            anyOf: _AnyOfVisitor.default,
            oneOf: _OneOfVisitor.default,
            not: _JSONSchemaOrJSONReferenceVisitor.default,
            definitions: _DefinitionsVisitor.default,
            // metadata keywords
            title: {
              $ref: '#/visitors/value'
            },
            description: {
              $ref: '#/visitors/value'
            },
            default: {
              $ref: '#/visitors/value'
            },
            // semantic validation with "format"
            format: {
              $ref: '#/visitors/value'
            },
            // JSON Hyper-Schema
            base: {
              $ref: '#/visitors/value'
            },
            links: _LinksVisitor.default,
            media: {
              $ref: '#/visitors/document/objects/Media'
            },
            readOnly: {
              $ref: '#/visitors/value'
            }
          }
        },
        JSONReference: {
          $visitor: _index2.default,
          fixedFields: {
            $ref: _$RefVisitor.default
          }
        },
        Media: {
          $visitor: _index3.default,
          fixedFields: {
            binaryEncoding: {
              $ref: '#/visitors/value'
            },
            type: {
              $ref: '#/visitors/value'
            }
          }
        },
        LinkDescription: {
          $visitor: _index4.default,
          fixedFields: {
            href: {
              $ref: '#/visitors/value'
            },
            rel: {
              $ref: '#/visitors/value'
            },
            title: {
              $ref: '#/visitors/value'
            },
            targetSchema: _JSONSchemaOrJSONReferenceVisitor.default,
            mediaType: {
              $ref: '#/visitors/value'
            },
            method: {
              $ref: '#/visitors/value'
            },
            encType: {
              $ref: '#/visitors/value'
            },
            schema: _JSONSchemaOrJSONReferenceVisitor.default
          }
        }
      }
    }
  }
};
var _default = exports.default = specification;