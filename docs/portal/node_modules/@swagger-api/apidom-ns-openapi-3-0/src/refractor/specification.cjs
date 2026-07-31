"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-4");
var _index = _interopRequireDefault(require("./visitors/open-api-3-0/index.cjs"));
var _OpenapiVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/OpenapiVisitor.cjs"));
var _SpecificationExtensionVisitor = _interopRequireDefault(require("./visitors/SpecificationExtensionVisitor.cjs"));
var _index2 = _interopRequireDefault(require("./visitors/open-api-3-0/info/index.cjs"));
var _VersionVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/info/VersionVisitor.cjs"));
var _index3 = _interopRequireDefault(require("./visitors/open-api-3-0/contact/index.cjs"));
var _index4 = _interopRequireDefault(require("./visitors/open-api-3-0/license/index.cjs"));
var _index5 = _interopRequireDefault(require("./visitors/open-api-3-0/link/index.cjs"));
var _OperationRefVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/link/OperationRefVisitor.cjs"));
var _OperationIdVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/link/OperationIdVisitor.cjs"));
var _ParametersVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/link/ParametersVisitor.cjs"));
var _index6 = _interopRequireDefault(require("./visitors/open-api-3-0/server/index.cjs"));
var _UrlVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/server/UrlVisitor.cjs"));
var _ServersVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/ServersVisitor.cjs"));
var _index7 = _interopRequireDefault(require("./visitors/open-api-3-0/server-variable/index.cjs"));
var _VariablesVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/server/VariablesVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("./visitors/FallbackVisitor.cjs"));
var _index8 = _interopRequireDefault(require("./visitors/open-api-3-0/media-type/index.cjs"));
var _SchemaVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/media-type/SchemaVisitor.cjs"));
var _ExamplesVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/media-type/ExamplesVisitor.cjs"));
var _EncodingVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/media-type/EncodingVisitor.cjs"));
var _index9 = _interopRequireDefault(require("./visitors/open-api-3-0/security-requirement/index.cjs"));
var _SecurityVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/SecurityVisitor.cjs"));
var _index0 = _interopRequireDefault(require("./visitors/open-api-3-0/components/index.cjs"));
var _index1 = _interopRequireDefault(require("./visitors/open-api-3-0/tag/index.cjs"));
var _index10 = _interopRequireDefault(require("./visitors/open-api-3-0/reference/index.cjs"));
var _$RefVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/reference/$RefVisitor.cjs"));
var _index11 = _interopRequireDefault(require("./visitors/open-api-3-0/parameter/index.cjs"));
var _SchemaVisitor2 = _interopRequireDefault(require("./visitors/open-api-3-0/parameter/SchemaVisitor.cjs"));
var _index12 = _interopRequireDefault(require("./visitors/open-api-3-0/header/index.cjs"));
var _SchemaVisitor3 = _interopRequireDefault(require("./visitors/open-api-3-0/header/SchemaVisitor.cjs"));
var _ExamplesVisitor2 = _interopRequireDefault(require("./visitors/open-api-3-0/header/ExamplesVisitor.cjs"));
var _ContentVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/header/ContentVisitor.cjs"));
var _index13 = _interopRequireDefault(require("./visitors/open-api-3-0/schema/index.cjs"));
var _AllOfVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/schema/AllOfVisitor.cjs"));
var _AnyOfVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/schema/AnyOfVisitor.cjs"));
var _OneOfVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/schema/OneOfVisitor.cjs"));
var _ItemsVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/schema/ItemsVisitor.cjs"));
var _PropertiesVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/schema/PropertiesVisitor.cjs"));
var _TypeVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/schema/TypeVisitor.cjs"));
var _SchemaOrReferenceVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/schema/SchemaOrReferenceVisitor.cjs"));
var _index14 = _interopRequireDefault(require("./visitors/open-api-3-0/distriminator/index.cjs"));
var _MappingVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/distriminator/MappingVisitor.cjs"));
var _index15 = _interopRequireDefault(require("./visitors/open-api-3-0/xml/index.cjs"));
var _ExamplesVisitor3 = _interopRequireDefault(require("./visitors/open-api-3-0/parameter/ExamplesVisitor.cjs"));
var _ContentVisitor2 = _interopRequireDefault(require("./visitors/open-api-3-0/parameter/ContentVisitor.cjs"));
var _SchemasVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/components/SchemasVisitor.cjs"));
var _ResponsesVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/components/ResponsesVisitor.cjs"));
var _ParametersVisitor2 = _interopRequireDefault(require("./visitors/open-api-3-0/components/ParametersVisitor.cjs"));
var _ExamplesVisitor4 = _interopRequireDefault(require("./visitors/open-api-3-0/components/ExamplesVisitor.cjs"));
var _RequestBodiesVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/components/RequestBodiesVisitor.cjs"));
var _HeadersVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/components/HeadersVisitor.cjs"));
var _SecuritySchemesVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/components/SecuritySchemesVisitor.cjs"));
var _LinksVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/components/LinksVisitor.cjs"));
var _CallbacksVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/components/CallbacksVisitor.cjs"));
var _index16 = _interopRequireDefault(require("./visitors/open-api-3-0/example/index.cjs"));
var _ExternalValueVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/example/ExternalValueVisitor.cjs"));
var _index17 = _interopRequireDefault(require("./visitors/open-api-3-0/external-documentation/index.cjs"));
var _index18 = _interopRequireDefault(require("./visitors/open-api-3-0/encoding/index.cjs"));
var _HeadersVisitor2 = _interopRequireDefault(require("./visitors/open-api-3-0/encoding/HeadersVisitor.cjs"));
var _index19 = _interopRequireDefault(require("./visitors/open-api-3-0/paths/index.cjs"));
var _index20 = _interopRequireDefault(require("./visitors/open-api-3-0/request-body/index.cjs"));
var _ContentVisitor3 = _interopRequireDefault(require("./visitors/open-api-3-0/request-body/ContentVisitor.cjs"));
var _index21 = _interopRequireDefault(require("./visitors/open-api-3-0/callback/index.cjs"));
var _index22 = _interopRequireDefault(require("./visitors/open-api-3-0/response/index.cjs"));
var _HeadersVisitor3 = _interopRequireDefault(require("./visitors/open-api-3-0/response/HeadersVisitor.cjs"));
var _ContentVisitor4 = _interopRequireDefault(require("./visitors/open-api-3-0/response/ContentVisitor.cjs"));
var _LinksVisitor2 = _interopRequireDefault(require("./visitors/open-api-3-0/response/LinksVisitor.cjs"));
var _index23 = _interopRequireDefault(require("./visitors/open-api-3-0/responses/index.cjs"));
var _DefaultVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/responses/DefaultVisitor.cjs"));
var _index24 = _interopRequireDefault(require("./visitors/open-api-3-0/operation/index.cjs"));
var _TagsVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/operation/TagsVisitor.cjs"));
var _ParametersVisitor3 = _interopRequireDefault(require("./visitors/open-api-3-0/operation/ParametersVisitor.cjs"));
var _RequestBodyVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/operation/RequestBodyVisitor.cjs"));
var _CallbacksVisitor2 = _interopRequireDefault(require("./visitors/open-api-3-0/operation/CallbacksVisitor.cjs"));
var _SecurityVisitor2 = _interopRequireDefault(require("./visitors/open-api-3-0/operation/SecurityVisitor.cjs"));
var _ServersVisitor2 = _interopRequireDefault(require("./visitors/open-api-3-0/operation/ServersVisitor.cjs"));
var _index25 = _interopRequireDefault(require("./visitors/open-api-3-0/path-item/index.cjs"));
var _$RefVisitor2 = _interopRequireDefault(require("./visitors/open-api-3-0/path-item/$RefVisitor.cjs"));
var _ServersVisitor3 = _interopRequireDefault(require("./visitors/open-api-3-0/path-item/ServersVisitor.cjs"));
var _ParametersVisitor4 = _interopRequireDefault(require("./visitors/open-api-3-0/path-item/ParametersVisitor.cjs"));
var _index26 = _interopRequireDefault(require("./visitors/open-api-3-0/security-scheme/index.cjs"));
var _index27 = _interopRequireDefault(require("./visitors/open-api-3-0/oauth-flows/index.cjs"));
var _index28 = _interopRequireDefault(require("./visitors/open-api-3-0/oauth-flow/index.cjs"));
var _ScopesVisitor = _interopRequireDefault(require("./visitors/open-api-3-0/oauth-flow/ScopesVisitor.cjs"));
var _TagsVisitor2 = _interopRequireDefault(require("./visitors/open-api-3-0/TagsVisitor.cjs"));
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
        OpenApi: {
          $visitor: _index.default,
          fixedFields: {
            openapi: _OpenapiVisitor.default,
            info: {
              $ref: '#/visitors/document/objects/Info'
            },
            servers: _ServersVisitor.default,
            paths: {
              $ref: '#/visitors/document/objects/Paths'
            },
            components: {
              $ref: '#/visitors/document/objects/Components'
            },
            security: _SecurityVisitor.default,
            tags: _TagsVisitor2.default,
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
        Server: {
          $visitor: _index6.default,
          fixedFields: {
            url: _UrlVisitor.default,
            description: {
              $ref: '#/visitors/value'
            },
            variables: _VariablesVisitor.default
          }
        },
        ServerVariable: {
          $visitor: _index7.default,
          fixedFields: {
            enum: {
              $ref: '#/visitors/value'
            },
            default: {
              $ref: '#/visitors/value'
            },
            description: {
              $ref: '#/visitors/value'
            }
          }
        },
        Components: {
          $visitor: _index0.default,
          fixedFields: {
            schemas: _SchemasVisitor.default,
            responses: _ResponsesVisitor.default,
            parameters: _ParametersVisitor2.default,
            examples: _ExamplesVisitor4.default,
            requestBodies: _RequestBodiesVisitor.default,
            headers: _HeadersVisitor.default,
            securitySchemes: _SecuritySchemesVisitor.default,
            links: _LinksVisitor.default,
            callbacks: _CallbacksVisitor.default
          }
        },
        Paths: {
          $visitor: _index19.default
        },
        PathItem: {
          $visitor: _index25.default,
          fixedFields: {
            $ref: _$RefVisitor2.default,
            summary: {
              $ref: '#/visitors/value'
            },
            description: {
              $ref: '#/visitors/value'
            },
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
            trace: {
              $ref: '#/visitors/document/objects/Operation'
            },
            servers: _ServersVisitor3.default,
            parameters: _ParametersVisitor4.default
          }
        },
        Operation: {
          $visitor: _index24.default,
          fixedFields: {
            tags: _TagsVisitor.default,
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
            parameters: _ParametersVisitor3.default,
            requestBody: _RequestBodyVisitor.default,
            responses: {
              $ref: '#/visitors/document/objects/Responses'
            },
            callbacks: _CallbacksVisitor2.default,
            deprecated: {
              $ref: '#/visitors/value'
            },
            security: _SecurityVisitor2.default,
            servers: _ServersVisitor2.default
          }
        },
        ExternalDocumentation: {
          $visitor: _index17.default,
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
          $visitor: _index11.default,
          fixedFields: {
            name: {
              $ref: '#/visitors/value'
            },
            in: {
              $ref: '#/visitors/value'
            },
            description: {
              $ref: '#/visitors/value'
            },
            required: {
              $ref: '#/visitors/value'
            },
            deprecated: {
              $ref: '#/visitors/value'
            },
            allowEmptyValue: {
              $ref: '#/visitors/value'
            },
            style: {
              $ref: '#/visitors/value'
            },
            explode: {
              $ref: '#/visitors/value'
            },
            allowReserved: {
              $ref: '#/visitors/value'
            },
            schema: _SchemaVisitor2.default,
            example: {
              $ref: '#/visitors/value'
            },
            examples: _ExamplesVisitor3.default,
            content: _ContentVisitor2.default
          }
        },
        RequestBody: {
          $visitor: _index20.default,
          fixedFields: {
            description: {
              $ref: '#/visitors/value'
            },
            content: _ContentVisitor3.default,
            required: {
              $ref: '#/visitors/value'
            }
          }
        },
        MediaType: {
          $visitor: _index8.default,
          fixedFields: {
            schema: _SchemaVisitor.default,
            example: {
              $ref: '#/visitors/value'
            },
            examples: _ExamplesVisitor.default,
            encoding: _EncodingVisitor.default
          }
        },
        Encoding: {
          $visitor: _index18.default,
          fixedFields: {
            contentType: {
              $ref: '#/visitors/value'
            },
            headers: _HeadersVisitor2.default,
            style: {
              $ref: '#/visitors/value'
            },
            explode: {
              $ref: '#/visitors/value'
            },
            allowReserved: {
              $ref: '#/visitors/value'
            }
          }
        },
        Responses: {
          $visitor: _index23.default,
          fixedFields: {
            default: _DefaultVisitor.default
          }
        },
        Response: {
          $visitor: _index22.default,
          fixedFields: {
            description: {
              $ref: '#/visitors/value'
            },
            headers: _HeadersVisitor3.default,
            content: _ContentVisitor4.default,
            links: _LinksVisitor2.default
          }
        },
        Callback: {
          $visitor: _index21.default
        },
        Example: {
          $visitor: _index16.default,
          fixedFields: {
            summary: {
              $ref: '#/visitors/value'
            },
            description: {
              $ref: '#/visitors/value'
            },
            value: {
              $ref: '#/visitors/value'
            },
            externalValue: _ExternalValueVisitor.default
          }
        },
        Link: {
          $visitor: _index5.default,
          fixedFields: {
            operationRef: _OperationRefVisitor.default,
            operationId: _OperationIdVisitor.default,
            parameters: _ParametersVisitor.default,
            requestBody: {
              $ref: '#/visitors/value'
            },
            description: {
              $ref: '#/visitors/value'
            },
            server: {
              $ref: '#/visitors/document/objects/Server'
            }
          }
        },
        Header: {
          $visitor: _index12.default,
          fixedFields: {
            description: {
              $ref: '#/visitors/value'
            },
            required: {
              $ref: '#/visitors/value'
            },
            deprecated: {
              $ref: '#/visitors/value'
            },
            allowEmptyValue: {
              $ref: '#/visitors/value'
            },
            style: {
              $ref: '#/visitors/value'
            },
            explode: {
              $ref: '#/visitors/value'
            },
            allowReserved: {
              $ref: '#/visitors/value'
            },
            schema: _SchemaVisitor3.default,
            example: {
              $ref: '#/visitors/value'
            },
            examples: _ExamplesVisitor2.default,
            content: _ContentVisitor.default
          }
        },
        Tag: {
          $visitor: _index1.default,
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
          $visitor: _index10.default,
          fixedFields: {
            $ref: _$RefVisitor.default
          }
        },
        JSONSchema: {
          $ref: '#/visitors/document/objects/Schema'
        },
        JSONReference: {
          $ref: '#/visitors/document/objects/Reference'
        },
        Schema: {
          $visitor: _index13.default,
          fixedFields: {
            // the following properties are taken directly from the JSON Schema definition and follow the same specifications
            title: jsonSchemaFixedFields.title,
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
            // the following properties are taken from the JSON Schema definition but their definitions were adjusted to the OpenAPI Specification
            type: _TypeVisitor.default,
            allOf: _AllOfVisitor.default,
            anyOf: _AnyOfVisitor.default,
            oneOf: _OneOfVisitor.default,
            not: _SchemaOrReferenceVisitor.default,
            items: _ItemsVisitor.default,
            properties: _PropertiesVisitor.default,
            additionalProperties: _SchemaOrReferenceVisitor.default,
            description: jsonSchemaFixedFields.description,
            format: jsonSchemaFixedFields.format,
            default: jsonSchemaFixedFields.default,
            // OpenAPI vocabulary
            nullable: {
              $ref: '#/visitors/value'
            },
            discriminator: {
              $ref: '#/visitors/document/objects/Discriminator'
            },
            writeOnly: {
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
            },
            deprecated: {
              $ref: '#/visitors/value'
            }
          }
        },
        Discriminator: {
          $visitor: _index14.default,
          fixedFields: {
            propertyName: {
              $ref: '#/visitors/value'
            },
            mapping: _MappingVisitor.default
          }
        },
        XML: {
          $visitor: _index15.default,
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
        SecurityScheme: {
          $visitor: _index26.default,
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
            scheme: {
              $ref: '#/visitors/value'
            },
            bearerFormat: {
              $ref: '#/visitors/value'
            },
            flows: {
              $ref: '#/visitors/document/objects/OAuthFlows'
            },
            openIdConnectUrl: {
              $ref: '#/visitors/value'
            }
          }
        },
        OAuthFlows: {
          $visitor: _index27.default,
          fixedFields: {
            implicit: {
              $ref: '#/visitors/document/objects/OAuthFlow'
            },
            password: {
              $ref: '#/visitors/document/objects/OAuthFlow'
            },
            clientCredentials: {
              $ref: '#/visitors/document/objects/OAuthFlow'
            },
            authorizationCode: {
              $ref: '#/visitors/document/objects/OAuthFlow'
            }
          }
        },
        OAuthFlow: {
          $visitor: _index28.default,
          fixedFields: {
            authorizationUrl: {
              $ref: '#/visitors/value'
            },
            tokenUrl: {
              $ref: '#/visitors/value'
            },
            refreshUrl: {
              $ref: '#/visitors/value'
            },
            scopes: _ScopesVisitor.default
          }
        },
        SecurityRequirement: {
          $visitor: _index9.default
        }
      },
      extension: {
        $visitor: _SpecificationExtensionVisitor.default
      }
    }
  }
};
var _default = exports.default = specification;