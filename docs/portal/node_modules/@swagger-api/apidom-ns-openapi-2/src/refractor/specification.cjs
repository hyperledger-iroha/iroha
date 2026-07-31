"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-4");
var _FallbackVisitor = _interopRequireDefault(require("./visitors/FallbackVisitor.cjs"));
var _index = _interopRequireDefault(require("./visitors/open-api-2/index.cjs"));
var _SwaggerVisitor = _interopRequireDefault(require("./visitors/open-api-2/SwaggerVisitor.cjs"));
var _HostVisitor = _interopRequireDefault(require("./visitors/open-api-2/HostVisitor.cjs"));
var _BasePathVisitor = _interopRequireDefault(require("./visitors/open-api-2/BasePathVisitor.cjs"));
var _SchemesVisitor = _interopRequireDefault(require("./visitors/open-api-2/SchemesVisitor.cjs"));
var _ConsumesVisitor = _interopRequireDefault(require("./visitors/open-api-2/ConsumesVisitor.cjs"));
var _ProducesVisitor = _interopRequireDefault(require("./visitors/open-api-2/ProducesVisitor.cjs"));
var _SecurityVisitor = _interopRequireDefault(require("./visitors/open-api-2/SecurityVisitor.cjs"));
var _TagsVisitor = _interopRequireDefault(require("./visitors/open-api-2/TagsVisitor.cjs"));
var _index2 = _interopRequireDefault(require("./visitors/open-api-2/info/index.cjs"));
var _VersionVisitor = _interopRequireDefault(require("./visitors/open-api-2/info/VersionVisitor.cjs"));
var _index3 = _interopRequireDefault(require("./visitors/open-api-2/contact/index.cjs"));
var _index4 = _interopRequireDefault(require("./visitors/open-api-2/license/index.cjs"));
var _index5 = _interopRequireDefault(require("./visitors/open-api-2/paths/index.cjs"));
var _index6 = _interopRequireDefault(require("./visitors/open-api-2/path-item/index.cjs"));
var _$RefVisitor = _interopRequireDefault(require("./visitors/open-api-2/path-item/$RefVisitor.cjs"));
var _ParametersVisitor = _interopRequireDefault(require("./visitors/open-api-2/path-item/ParametersVisitor.cjs"));
var _index7 = _interopRequireDefault(require("./visitors/open-api-2/operation/index.cjs"));
var _TagsVisitor2 = _interopRequireDefault(require("./visitors/open-api-2/operation/TagsVisitor.cjs"));
var _ConsumesVisitor2 = _interopRequireDefault(require("./visitors/open-api-2/operation/ConsumesVisitor.cjs"));
var _ProducesVisitor2 = _interopRequireDefault(require("./visitors/open-api-2/operation/ProducesVisitor.cjs"));
var _ParametersVisitor2 = _interopRequireDefault(require("./visitors/open-api-2/operation/ParametersVisitor.cjs"));
var _SchemesVisitor2 = _interopRequireDefault(require("./visitors/open-api-2/operation/SchemesVisitor.cjs"));
var _SecurityVisitor2 = _interopRequireDefault(require("./visitors/open-api-2/operation/SecurityVisitor.cjs"));
var _index8 = _interopRequireDefault(require("./visitors/open-api-2/external-documentation/index.cjs"));
var _index9 = _interopRequireDefault(require("./visitors/open-api-2/parameter/index.cjs"));
var _index0 = _interopRequireDefault(require("./visitors/open-api-2/items/index.cjs"));
var _index1 = _interopRequireDefault(require("./visitors/open-api-2/responses/index.cjs"));
var _DefaultVisitor = _interopRequireDefault(require("./visitors/open-api-2/responses/DefaultVisitor.cjs"));
var _index10 = _interopRequireDefault(require("./visitors/open-api-2/response/index.cjs"));
var _index11 = _interopRequireDefault(require("./visitors/open-api-2/headers/index.cjs"));
var _index12 = _interopRequireDefault(require("./visitors/open-api-2/example/index.cjs"));
var _index13 = _interopRequireDefault(require("./visitors/open-api-2/header/index.cjs"));
var _index14 = _interopRequireDefault(require("./visitors/open-api-2/tag/index.cjs"));
var _index15 = _interopRequireDefault(require("./visitors/open-api-2/reference/index.cjs"));
var _$RefVisitor2 = _interopRequireDefault(require("./visitors/open-api-2/reference/$RefVisitor.cjs"));
var _index16 = _interopRequireDefault(require("./visitors/open-api-2/schema/index.cjs"));
var _AllOfVisitor = _interopRequireDefault(require("./visitors/open-api-2/schema/AllOfVisitor.cjs"));
var _ItemsVisitor = _interopRequireDefault(require("./visitors/open-api-2/schema/ItemsVisitor.cjs"));
var _PropertiesVisitor = _interopRequireDefault(require("./visitors/open-api-2/schema/PropertiesVisitor.cjs"));
var _SchemaOrJSONReferenceVisitor = _interopRequireDefault(require("./visitors/open-api-2/schema/SchemaOrJSONReferenceVisitor.cjs"));
var _index17 = _interopRequireDefault(require("./visitors/open-api-2/xml/index.cjs"));
var _index18 = _interopRequireDefault(require("./visitors/open-api-2/definitions/index.cjs"));
var _index19 = _interopRequireDefault(require("./visitors/open-api-2/responses-definitions/index.cjs"));
var _index20 = _interopRequireDefault(require("./visitors/open-api-2/parameters-definitions/index.cjs"));
var _index21 = _interopRequireDefault(require("./visitors/open-api-2/security-definitions/index.cjs"));
var _index22 = _interopRequireDefault(require("./visitors/open-api-2/security-scheme/index.cjs"));
var _index23 = _interopRequireDefault(require("./visitors/open-api-2/scopes/index.cjs"));
var _index24 = _interopRequireDefault(require("./visitors/open-api-2/security-requirement/index.cjs"));
var _SpecificationExtensionVisitor = _interopRequireDefault(require("./visitors/SpecificationExtensionVisitor.cjs"));
/**
 * Specification object allows us to have complete control over visitors
 * when traversing the ApiDOM.
 * Specification also allows us to create amended refractors from
 * existing ones by manipulating it.
 *
 * Note: Specification object allows to use absolute internal JSON pointers.
 */

const {
  fixedFields: jsonSchemaFixedFields
} = _apidomNsJsonSchemaDraft.specificationObj.visitors.document.objects.JSONSchema;

/**
 * @public
 */
const specification = {
  visitors: {
    value: _FallbackVisitor.default,
    document: {
      objects: {
        // JSON Schema Draft 4/5 specific visitors
        JSONReference: _apidomNsJsonSchemaDraft.specificationObj.visitors.document.objects.JSONReference,
        JSONSchema: {
          $ref: '#/visitors/document/objects/Schema'
        },
        // OpenAPI 2 specific visitors
        Swagger: {
          $visitor: _index.default,
          fixedFields: {
            swagger: _SwaggerVisitor.default,
            info: {
              $ref: '#/visitors/document/objects/Info'
            },
            host: _HostVisitor.default,
            basePath: _BasePathVisitor.default,
            schemes: _SchemesVisitor.default,
            consumes: _ConsumesVisitor.default,
            produces: _ProducesVisitor.default,
            paths: {
              $ref: '#/visitors/document/objects/Paths'
            },
            definitions: {
              $ref: '#/visitors/document/objects/Definitions'
            },
            parameters: {
              $ref: '#/visitors/document/objects/ParametersDefinitions'
            },
            responses: {
              $ref: '#/visitors/document/objects/ResponsesDefinitions'
            },
            securityDefinitions: {
              $ref: '#/visitors/document/objects/SecurityDefinitions'
            },
            security: _SecurityVisitor.default,
            tags: _TagsVisitor.default,
            externalDocs: {
              $ref: '#/visitors/document/objects/ExternalDocumentation'
            }
          }
        },
        Info: {
          $visitor: _index2.default,
          fixedFields: {
            title: {
              $ref: '#/visitors/value'
            },
            description: {
              $ref: '#/visitors/value'
            },
            termsOfService: {
              $ref: '#/visitors/value'
            },
            contact: {
              $ref: '#/visitors/document/objects/Contact'
            },
            license: {
              $ref: '#/visitors/document/objects/License'
            },
            version: _VersionVisitor.default
          }
        },
        Contact: {
          $visitor: _index3.default,
          fixedFields: {
            name: {
              $ref: '#/visitors/value'
            },
            url: {
              $ref: '#/visitors/value'
            },
            email: {
              $ref: '#/visitors/value'
            }
          }
        },
        License: {
          $visitor: _index4.default,
          fixedFields: {
            name: {
              $ref: '#/visitors/value'
            },
            url: {
              $ref: '#/visitors/value'
            }
          }
        },
        Paths: {
          $visitor: _index5.default
        },
        PathItem: {
          $visitor: _index6.default,
          fixedFields: {
            $ref: _$RefVisitor.default,
            get: {
              $ref: '#/visitors/document/objects/Operation'
            },
            put: {
              $ref: '#/visitors/document/objects/Operation'
            },
            post: {
              $ref: '#/visitors/document/objects/Operation'
            },
            delete: {
              $ref: '#/visitors/document/objects/Operation'
            },
            options: {
              $ref: '#/visitors/document/objects/Operation'
            },
            head: {
              $ref: '#/visitors/document/objects/Operation'
            },
            patch: {
              $ref: '#/visitors/document/objects/Operation'
            },
            parameters: _ParametersVisitor.default
          }
        },
        Operation: {
          $visitor: _index7.default,
          fixedFields: {
            tags: _TagsVisitor2.default,
            summary: {
              $ref: '#/visitors/value'
            },
            description: {
              $ref: '#/visitors/value'
            },
            externalDocs: {
              $ref: '#/visitors/document/objects/ExternalDocumentation'
            },
            operationId: {
              $ref: '#/visitors/value'
            },
            consumes: _ConsumesVisitor2.default,
            produces: _ProducesVisitor2.default,
            parameters: _ParametersVisitor2.default,
            responses: {
              $ref: '#/visitors/document/objects/Responses'
            },
            schemes: _SchemesVisitor2.default,
            deprecated: {
              $ref: '#/visitors/value'
            },
            security: _SecurityVisitor2.default
          }
        },
        ExternalDocumentation: {
          $visitor: _index8.default,
          fixedFields: {
            description: {
              $ref: '#/visitors/value'
            },
            url: {
              $ref: '#/visitors/value'
            }
          }
        },
        Parameter: {
          $visitor: _index9.default,
          fixedFields: {
            name: {
              $ref: '#/visitors/value'
            },
            in: {
              $ref: '#/visitors/value'
            },
            description: jsonSchemaFixedFields.description,
            required: jsonSchemaFixedFields.required,
            schema: _SchemaOrJSONReferenceVisitor.default,
            type: jsonSchemaFixedFields.type,
            format: jsonSchemaFixedFields.format,
            items: {
              $ref: '#/visitors/document/objects/Items'
            },
            collectionFormat: {
              $ref: '#/visitors/value'
            },
            default: jsonSchemaFixedFields.default,
            maximum: jsonSchemaFixedFields.maximum,
            exclusiveMaximum: jsonSchemaFixedFields.exclusiveMaximum,
            minimum: jsonSchemaFixedFields.minimum,
            exclusiveMinimum: jsonSchemaFixedFields.exclusiveMinimum,
            maxLength: jsonSchemaFixedFields.maxLength,
            minLength: jsonSchemaFixedFields.minLength,
            pattern: jsonSchemaFixedFields.pattern,
            maxItems: jsonSchemaFixedFields.maxItems,
            minItems: jsonSchemaFixedFields.minItems,
            uniqueItems: jsonSchemaFixedFields.uniqueItems,
            enum: jsonSchemaFixedFields.enum,
            multipleOf: jsonSchemaFixedFields.multipleOf
          }
        },
        Items: {
          $visitor: _index0.default,
          fixedFields: {
            type: jsonSchemaFixedFields.type,
            format: jsonSchemaFixedFields.format,
            items: {
              $ref: '#/visitors/document/objects/Items'
            },
            collectionFormat: {
              $ref: '#/visitors/value'
            },
            default: jsonSchemaFixedFields.default,
            maximum: jsonSchemaFixedFields.maximum,
            exclusiveMaximum: jsonSchemaFixedFields.exclusiveMaximum,
            minimum: jsonSchemaFixedFields.minimum,
            exclusiveMinimum: jsonSchemaFixedFields.exclusiveMinimum,
            maxLength: jsonSchemaFixedFields.maxLength,
            minLength: jsonSchemaFixedFields.minLength,
            pattern: jsonSchemaFixedFields.pattern,
            maxItems: jsonSchemaFixedFields.maxItems,
            minItems: jsonSchemaFixedFields.minItems,
            uniqueItems: jsonSchemaFixedFields.uniqueItems,
            enum: jsonSchemaFixedFields.enum,
            multipleOf: jsonSchemaFixedFields.multipleOf
          }
        },
        Responses: {
          $visitor: _index1.default,
          fixedFields: {
            default: _DefaultVisitor.default
          }
        },
        Response: {
          $visitor: _index10.default,
          fixedFields: {
            description: {
              $ref: '#/visitors/value'
            },
            schema: _SchemaOrJSONReferenceVisitor.default,
            headers: {
              $ref: '#/visitors/document/objects/Headers'
            },
            examples: {
              $ref: '#/visitors/document/objects/Example'
            }
          }
        },
        Headers: {
          $visitor: _index11.default
        },
        Example: {
          $visitor: _index12.default
        },
        Header: {
          $visitor: _index13.default,
          fixedFields: {
            description: jsonSchemaFixedFields.description,
            type: jsonSchemaFixedFields.type,
            format: jsonSchemaFixedFields.format,
            items: {
              $ref: '#/visitors/document/objects/Items'
            },
            collectionFormat: {
              $ref: '#/visitors/value'
            },
            default: jsonSchemaFixedFields.default,
            maximum: jsonSchemaFixedFields.maximum,
            exclusiveMaximum: jsonSchemaFixedFields.exclusiveMaximum,
            minimum: jsonSchemaFixedFields.minimum,
            exclusiveMinimum: jsonSchemaFixedFields.exclusiveMinimum,
            maxLength: jsonSchemaFixedFields.maxLength,
            minLength: jsonSchemaFixedFields.minLength,
            pattern: jsonSchemaFixedFields.pattern,
            maxItems: jsonSchemaFixedFields.maxItems,
            minItems: jsonSchemaFixedFields.minItems,
            uniqueItems: jsonSchemaFixedFields.uniqueItems,
            enum: jsonSchemaFixedFields.enum,
            multipleOf: jsonSchemaFixedFields.multipleOf
          }
        },
        Tag: {
          $visitor: _index14.default,
          fixedFields: {
            name: {
              $ref: '#/visitors/value'
            },
            description: {
              $ref: '#/visitors/value'
            },
            externalDocs: {
              $ref: '#/visitors/document/objects/ExternalDocumentation'
            }
          }
        },
        Reference: {
          $visitor: _index15.default,
          fixedFields: {
            $ref: _$RefVisitor2.default
          }
        },
        Schema: {
          $visitor: _index16.default,
          fixedFields: {
            // the following properties are taken directly from the JSON Schema definition and follow the same specifications
            format: jsonSchemaFixedFields.format,
            title: jsonSchemaFixedFields.title,
            description: jsonSchemaFixedFields.description,
            default: jsonSchemaFixedFields.default,
            multipleOf: jsonSchemaFixedFields.multipleOf,
            maximum: jsonSchemaFixedFields.maximum,
            exclusiveMaximum: jsonSchemaFixedFields.exclusiveMaximum,
            minimum: jsonSchemaFixedFields.minimum,
            exclusiveMinimum: jsonSchemaFixedFields.exclusiveMinimum,
            maxLength: jsonSchemaFixedFields.maxLength,
            minLength: jsonSchemaFixedFields.minLength,
            pattern: jsonSchemaFixedFields.pattern,
            maxItems: jsonSchemaFixedFields.maxItems,
            minItems: jsonSchemaFixedFields.minItems,
            uniqueItems: jsonSchemaFixedFields.uniqueItems,
            maxProperties: jsonSchemaFixedFields.maxProperties,
            minProperties: jsonSchemaFixedFields.minProperties,
            required: jsonSchemaFixedFields.required,
            enum: jsonSchemaFixedFields.enum,
            type: jsonSchemaFixedFields.type,
            readOnly: jsonSchemaFixedFields.readOnly,
            // the following properties are taken from the JSON Schema definition but their definitions were adjusted to the Swagger Specification
            items: _ItemsVisitor.default,
            allOf: _AllOfVisitor.default,
            properties: _PropertiesVisitor.default,
            additionalProperties: _SchemaOrJSONReferenceVisitor.default,
            // OpenAPI vocabulary
            discriminator: {
              $ref: '#/visitors/value'
            },
            xml: {
              $ref: '#/visitors/document/objects/XML'
            },
            externalDocs: {
              $ref: '#/visitors/document/objects/ExternalDocumentation'
            },
            example: {
              $ref: '#/visitors/value'
            }
          }
        },
        XML: {
          $visitor: _index17.default,
          fixedFields: {
            name: {
              $ref: '#/visitors/value'
            },
            namespace: {
              $ref: '#/visitors/value'
            },
            prefix: {
              $ref: '#/visitors/value'
            },
            attribute: {
              $ref: '#/visitors/value'
            },
            wrapped: {
              $ref: '#/visitors/value'
            }
          }
        },
        Definitions: {
          $visitor: _index18.default
        },
        ParametersDefinitions: {
          $visitor: _index20.default
        },
        ResponsesDefinitions: {
          $visitor: _index19.default
        },
        SecurityDefinitions: {
          $visitor: _index21.default
        },
        SecurityScheme: {
          $visitor: _index22.default,
          fixedFields: {
            type: {
              $ref: '#/visitors/value'
            },
            description: {
              $ref: '#/visitors/value'
            },
            name: {
              $ref: '#/visitors/value'
            },
            in: {
              $ref: '#/visitors/value'
            },
            flow: {
              $ref: '#/visitors/value'
            },
            authorizationUrl: {
              $ref: '#/visitors/value'
            },
            token: {
              $ref: '#/visitors/value'
            },
            scopes: {
              $ref: '#/visitors/document/objects/Scopes'
            }
          }
        },
        Scopes: {
          $visitor: _index23.default
        },
        SecurityRequirement: {
          $visitor: _index24.default
        }
      },
      extension: {
        $visitor: _SpecificationExtensionVisitor.default
      }
    }
  }
};
var _default = exports.default = specification;