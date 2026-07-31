"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _apidomNsJsonSchema = require("@swagger-api/apidom-ns-json-schema-2020-12");
var _index = _interopRequireDefault(require("./visitors/open-api-3-1/index.cjs"));
var _index2 = _interopRequireDefault(require("./visitors/open-api-3-1/info/index.cjs"));
var _index3 = _interopRequireDefault(require("./visitors/open-api-3-1/contact/index.cjs"));
var _index4 = _interopRequireDefault(require("./visitors/open-api-3-1/license/index.cjs"));
var _index5 = _interopRequireDefault(require("./visitors/open-api-3-1/link/index.cjs"));
var _JsonSchemaDialectVisitor = _interopRequireDefault(require("./visitors/open-api-3-1/JsonSchemaDialectVisitor.cjs"));
var _index6 = _interopRequireDefault(require("./visitors/open-api-3-1/server/index.cjs"));
var _index7 = _interopRequireDefault(require("./visitors/open-api-3-1/server-variable/index.cjs"));
var _index8 = _interopRequireDefault(require("./visitors/open-api-3-1/media-type/index.cjs"));
var _index9 = _interopRequireDefault(require("./visitors/open-api-3-1/security-requirement/index.cjs"));
var _index0 = _interopRequireDefault(require("./visitors/open-api-3-1/components/index.cjs"));
var _index1 = _interopRequireDefault(require("./visitors/open-api-3-1/tag/index.cjs"));
var _index10 = _interopRequireDefault(require("./visitors/open-api-3-1/reference/index.cjs"));
var _index11 = _interopRequireDefault(require("./visitors/open-api-3-1/parameter/index.cjs"));
var _index12 = _interopRequireDefault(require("./visitors/open-api-3-1/header/index.cjs"));
var _index13 = _interopRequireDefault(require("./visitors/open-api-3-1/schema/index.cjs"));
var _$defsVisitor = _interopRequireDefault(require("./visitors/open-api-3-1/schema/$defsVisitor.cjs"));
var _AllOfVisitor = _interopRequireDefault(require("./visitors/open-api-3-1/schema/AllOfVisitor.cjs"));
var _AnyOfVisitor = _interopRequireDefault(require("./visitors/open-api-3-1/schema/AnyOfVisitor.cjs"));
var _OneOfVisitor = _interopRequireDefault(require("./visitors/open-api-3-1/schema/OneOfVisitor.cjs"));
var _DependentSchemasVisitor = _interopRequireDefault(require("./visitors/open-api-3-1/schema/DependentSchemasVisitor.cjs"));
var _PrefixItemsVisitor = _interopRequireDefault(require("./visitors/open-api-3-1/schema/PrefixItemsVisitor.cjs"));
var _PropertiesVisitor = _interopRequireDefault(require("./visitors/open-api-3-1/schema/PropertiesVisitor.cjs"));
var _PatternPropertiesVisitor = _interopRequireDefault(require("./visitors/open-api-3-1/schema/PatternPropertiesVisitor.cjs"));
var _index14 = _interopRequireDefault(require("./visitors/open-api-3-1/distriminator/index.cjs"));
var _index15 = _interopRequireDefault(require("./visitors/open-api-3-1/xml/index.cjs"));
var _SchemasVisitor = _interopRequireDefault(require("./visitors/open-api-3-1/components/SchemasVisitor.cjs"));
var _PathItemsVisitor = _interopRequireDefault(require("./visitors/open-api-3-1/components/PathItemsVisitor.cjs"));
var _index16 = _interopRequireDefault(require("./visitors/open-api-3-1/example/index.cjs"));
var _index17 = _interopRequireDefault(require("./visitors/open-api-3-1/external-documentation/index.cjs"));
var _index18 = _interopRequireDefault(require("./visitors/open-api-3-1/encoding/index.cjs"));
var _index19 = _interopRequireDefault(require("./visitors/open-api-3-1/paths/index.cjs"));
var _index20 = _interopRequireDefault(require("./visitors/open-api-3-1/request-body/index.cjs"));
var _index21 = _interopRequireDefault(require("./visitors/open-api-3-1/callback/index.cjs"));
var _index22 = _interopRequireDefault(require("./visitors/open-api-3-1/response/index.cjs"));
var _index23 = _interopRequireDefault(require("./visitors/open-api-3-1/responses/index.cjs"));
var _index24 = _interopRequireDefault(require("./visitors/open-api-3-1/operation/index.cjs"));
var _index25 = _interopRequireDefault(require("./visitors/open-api-3-1/path-item/index.cjs"));
var _index26 = _interopRequireDefault(require("./visitors/open-api-3-1/security-scheme/index.cjs"));
var _index27 = _interopRequireDefault(require("./visitors/open-api-3-1/oauth-flows/index.cjs"));
var _index28 = _interopRequireDefault(require("./visitors/open-api-3-1/oauth-flow/index.cjs"));
var _WebhooksVisitor = _interopRequireDefault(require("./visitors/open-api-3-1/WebhooksVisitor.cjs"));
const {
  JSONSchema: JSONSchemaVisitor,
  LinkDescription: LinkDescriptionVisitor
} = _apidomNsJsonSchema.specificationObj.visitors.document.objects;

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
    value: _apidomNsOpenapi.specificationObj.visitors.value,
    document: {
      objects: {
        OpenApi: {
          $visitor: _index.default,
          fixedFields: {
            openapi: _apidomNsOpenapi.specificationObj.visitors.document.objects.OpenApi.fixedFields.openapi,
            info: {
              $ref: '#/visitors/document/objects/Info'
            },
            jsonSchemaDialect: _JsonSchemaDialectVisitor.default,
            servers: _apidomNsOpenapi.specificationObj.visitors.document.objects.OpenApi.fixedFields.servers,
            paths: {
              $ref: '#/visitors/document/objects/Paths'
            },
            webhooks: _WebhooksVisitor.default,
            components: {
              $ref: '#/visitors/document/objects/Components'
            },
            security: _apidomNsOpenapi.specificationObj.visitors.document.objects.OpenApi.fixedFields.security,
            tags: _apidomNsOpenapi.specificationObj.visitors.document.objects.OpenApi.fixedFields.tags,
            externalDocs: {
              $ref: '#/visitors/document/objects/ExternalDocumentation'
            }
          }
        },
        Info: {
          $visitor: _index2.default,
          fixedFields: {
            title: _apidomNsOpenapi.specificationObj.visitors.document.objects.Info.fixedFields.title,
            description: _apidomNsOpenapi.specificationObj.visitors.document.objects.Info.fixedFields.description,
            summary: {
              $ref: '#/visitors/value'
            },
            termsOfService: _apidomNsOpenapi.specificationObj.visitors.document.objects.Info.fixedFields.termsOfService,
            contact: {
              $ref: '#/visitors/document/objects/Contact'
            },
            license: {
              $ref: '#/visitors/document/objects/License'
            },
            version: _apidomNsOpenapi.specificationObj.visitors.document.objects.Info.fixedFields.version
          }
        },
        Contact: {
          $visitor: _index3.default,
          fixedFields: {
            name: _apidomNsOpenapi.specificationObj.visitors.document.objects.Contact.fixedFields.name,
            url: _apidomNsOpenapi.specificationObj.visitors.document.objects.Contact.fixedFields.url,
            email: _apidomNsOpenapi.specificationObj.visitors.document.objects.Contact.fixedFields.email
          }
        },
        License: {
          $visitor: _index4.default,
          fixedFields: {
            name: _apidomNsOpenapi.specificationObj.visitors.document.objects.License.fixedFields.name,
            identifier: {
              $ref: '#/visitors/value'
            },
            url: _apidomNsOpenapi.specificationObj.visitors.document.objects.License.fixedFields.url
          }
        },
        Server: {
          $visitor: _index6.default,
          fixedFields: {
            url: _apidomNsOpenapi.specificationObj.visitors.document.objects.Server.fixedFields.url,
            description: _apidomNsOpenapi.specificationObj.visitors.document.objects.Server.fixedFields.description,
            variables: _apidomNsOpenapi.specificationObj.visitors.document.objects.Server.fixedFields.variables
          }
        },
        ServerVariable: {
          $visitor: _index7.default,
          fixedFields: {
            enum: _apidomNsOpenapi.specificationObj.visitors.document.objects.ServerVariable.fixedFields.enum,
            default: _apidomNsOpenapi.specificationObj.visitors.document.objects.ServerVariable.fixedFields.default,
            description: _apidomNsOpenapi.specificationObj.visitors.document.objects.ServerVariable.fixedFields.description
          }
        },
        Components: {
          $visitor: _index0.default,
          fixedFields: {
            schemas: _SchemasVisitor.default,
            responses: _apidomNsOpenapi.specificationObj.visitors.document.objects.Components.fixedFields.responses,
            parameters: _apidomNsOpenapi.specificationObj.visitors.document.objects.Components.fixedFields.parameters,
            examples: _apidomNsOpenapi.specificationObj.visitors.document.objects.Components.fixedFields.examples,
            requestBodies: _apidomNsOpenapi.specificationObj.visitors.document.objects.Components.fixedFields.requestBodies,
            headers: _apidomNsOpenapi.specificationObj.visitors.document.objects.Components.fixedFields.headers,
            securitySchemes: _apidomNsOpenapi.specificationObj.visitors.document.objects.Components.fixedFields.securitySchemes,
            links: _apidomNsOpenapi.specificationObj.visitors.document.objects.Components.fixedFields.links,
            callbacks: _apidomNsOpenapi.specificationObj.visitors.document.objects.Components.fixedFields.callbacks,
            pathItems: _PathItemsVisitor.default
          }
        },
        Paths: {
          $visitor: _index19.default
        },
        PathItem: {
          $visitor: _index25.default,
          fixedFields: {
            $ref: _apidomNsOpenapi.specificationObj.visitors.document.objects.PathItem.fixedFields.$ref,
            summary: _apidomNsOpenapi.specificationObj.visitors.document.objects.PathItem.fixedFields.summary,
            description: _apidomNsOpenapi.specificationObj.visitors.document.objects.PathItem.fixedFields.description,
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
            servers: _apidomNsOpenapi.specificationObj.visitors.document.objects.PathItem.fixedFields.servers,
            parameters: _apidomNsOpenapi.specificationObj.visitors.document.objects.PathItem.fixedFields.parameters
          }
        },
        Operation: {
          $visitor: _index24.default,
          fixedFields: {
            tags: _apidomNsOpenapi.specificationObj.visitors.document.objects.Operation.fixedFields.tags,
            summary: _apidomNsOpenapi.specificationObj.visitors.document.objects.Operation.fixedFields.summary,
            description: _apidomNsOpenapi.specificationObj.visitors.document.objects.Operation.fixedFields.description,
            externalDocs: {
              $ref: '#/visitors/document/objects/ExternalDocumentation'
            },
            operationId: _apidomNsOpenapi.specificationObj.visitors.document.objects.Operation.fixedFields.operationId,
            parameters: _apidomNsOpenapi.specificationObj.visitors.document.objects.Operation.fixedFields.parameters,
            requestBody: _apidomNsOpenapi.specificationObj.visitors.document.objects.Operation.fixedFields.requestBody,
            responses: {
              $ref: '#/visitors/document/objects/Responses'
            },
            callbacks: _apidomNsOpenapi.specificationObj.visitors.document.objects.Operation.fixedFields.callbacks,
            deprecated: _apidomNsOpenapi.specificationObj.visitors.document.objects.Operation.fixedFields.deprecated,
            security: _apidomNsOpenapi.specificationObj.visitors.document.objects.Operation.fixedFields.security,
            servers: _apidomNsOpenapi.specificationObj.visitors.document.objects.Operation.fixedFields.servers
          }
        },
        ExternalDocumentation: {
          $visitor: _index17.default,
          fixedFields: {
            description: _apidomNsOpenapi.specificationObj.visitors.document.objects.ExternalDocumentation.fixedFields.description,
            url: _apidomNsOpenapi.specificationObj.visitors.document.objects.ExternalDocumentation.fixedFields.url
          }
        },
        Parameter: {
          $visitor: _index11.default,
          fixedFields: {
            name: _apidomNsOpenapi.specificationObj.visitors.document.objects.Parameter.fixedFields.name,
            in: _apidomNsOpenapi.specificationObj.visitors.document.objects.Parameter.fixedFields.in,
            description: _apidomNsOpenapi.specificationObj.visitors.document.objects.Parameter.fixedFields.description,
            required: _apidomNsOpenapi.specificationObj.visitors.document.objects.Parameter.fixedFields.required,
            deprecated: _apidomNsOpenapi.specificationObj.visitors.document.objects.Parameter.fixedFields.deprecated,
            allowEmptyValue: _apidomNsOpenapi.specificationObj.visitors.document.objects.Parameter.fixedFields.allowEmptyValue,
            style: _apidomNsOpenapi.specificationObj.visitors.document.objects.Parameter.fixedFields.style,
            explode: _apidomNsOpenapi.specificationObj.visitors.document.objects.Parameter.fixedFields.explode,
            allowReserved: _apidomNsOpenapi.specificationObj.visitors.document.objects.Parameter.fixedFields.allowReserved,
            schema: {
              $ref: '#/visitors/document/objects/Schema'
            },
            example: _apidomNsOpenapi.specificationObj.visitors.document.objects.Parameter.fixedFields.example,
            examples: _apidomNsOpenapi.specificationObj.visitors.document.objects.Parameter.fixedFields.examples,
            content: _apidomNsOpenapi.specificationObj.visitors.document.objects.Parameter.fixedFields.content
          }
        },
        RequestBody: {
          $visitor: _index20.default,
          fixedFields: {
            description: _apidomNsOpenapi.specificationObj.visitors.document.objects.RequestBody.fixedFields.description,
            content: _apidomNsOpenapi.specificationObj.visitors.document.objects.RequestBody.fixedFields.content,
            required: _apidomNsOpenapi.specificationObj.visitors.document.objects.RequestBody.fixedFields.required
          }
        },
        MediaType: {
          $visitor: _index8.default,
          fixedFields: {
            schema: {
              $ref: '#/visitors/document/objects/Schema'
            },
            example: _apidomNsOpenapi.specificationObj.visitors.document.objects.MediaType.fixedFields.example,
            examples: _apidomNsOpenapi.specificationObj.visitors.document.objects.MediaType.fixedFields.examples,
            encoding: _apidomNsOpenapi.specificationObj.visitors.document.objects.MediaType.fixedFields.encoding
          }
        },
        Encoding: {
          $visitor: _index18.default,
          fixedFields: {
            contentType: _apidomNsOpenapi.specificationObj.visitors.document.objects.Encoding.fixedFields.contentType,
            headers: _apidomNsOpenapi.specificationObj.visitors.document.objects.Encoding.fixedFields.headers,
            style: _apidomNsOpenapi.specificationObj.visitors.document.objects.Encoding.fixedFields.style,
            explode: _apidomNsOpenapi.specificationObj.visitors.document.objects.Encoding.fixedFields.explode,
            allowReserved: _apidomNsOpenapi.specificationObj.visitors.document.objects.Encoding.fixedFields.allowReserved
          }
        },
        Responses: {
          $visitor: _index23.default,
          fixedFields: {
            default: _apidomNsOpenapi.specificationObj.visitors.document.objects.Responses.fixedFields.default
          }
        },
        Response: {
          $visitor: _index22.default,
          fixedFields: {
            description: _apidomNsOpenapi.specificationObj.visitors.document.objects.Response.fixedFields.description,
            headers: _apidomNsOpenapi.specificationObj.visitors.document.objects.Response.fixedFields.headers,
            content: _apidomNsOpenapi.specificationObj.visitors.document.objects.Response.fixedFields.content,
            links: _apidomNsOpenapi.specificationObj.visitors.document.objects.Response.fixedFields.links
          }
        },
        Callback: {
          $visitor: _index21.default
        },
        Example: {
          $visitor: _index16.default,
          fixedFields: {
            summary: _apidomNsOpenapi.specificationObj.visitors.document.objects.Example.fixedFields.summary,
            description: _apidomNsOpenapi.specificationObj.visitors.document.objects.Example.fixedFields.description,
            value: _apidomNsOpenapi.specificationObj.visitors.document.objects.Example.fixedFields.value,
            externalValue: _apidomNsOpenapi.specificationObj.visitors.document.objects.Example.fixedFields.externalValue
          }
        },
        Link: {
          $visitor: _index5.default,
          fixedFields: {
            operationRef: _apidomNsOpenapi.specificationObj.visitors.document.objects.Link.fixedFields.operationRef,
            operationId: _apidomNsOpenapi.specificationObj.visitors.document.objects.Link.fixedFields.operationId,
            parameters: _apidomNsOpenapi.specificationObj.visitors.document.objects.Link.fixedFields.parameters,
            requestBody: _apidomNsOpenapi.specificationObj.visitors.document.objects.Link.fixedFields.requestBody,
            description: _apidomNsOpenapi.specificationObj.visitors.document.objects.Link.fixedFields.description,
            server: {
              $ref: '#/visitors/document/objects/Server'
            }
          }
        },
        Header: {
          $visitor: _index12.default,
          fixedFields: {
            description: _apidomNsOpenapi.specificationObj.visitors.document.objects.Header.fixedFields.description,
            required: _apidomNsOpenapi.specificationObj.visitors.document.objects.Header.fixedFields.required,
            deprecated: _apidomNsOpenapi.specificationObj.visitors.document.objects.Header.fixedFields.deprecated,
            allowEmptyValue: _apidomNsOpenapi.specificationObj.visitors.document.objects.Header.fixedFields.allowEmptyValue,
            style: _apidomNsOpenapi.specificationObj.visitors.document.objects.Header.fixedFields.style,
            explode: _apidomNsOpenapi.specificationObj.visitors.document.objects.Header.fixedFields.explode,
            allowReserved: _apidomNsOpenapi.specificationObj.visitors.document.objects.Header.fixedFields.allowReserved,
            schema: {
              $ref: '#/visitors/document/objects/Schema'
            },
            example: _apidomNsOpenapi.specificationObj.visitors.document.objects.Header.fixedFields.example,
            examples: _apidomNsOpenapi.specificationObj.visitors.document.objects.Header.fixedFields.examples,
            content: _apidomNsOpenapi.specificationObj.visitors.document.objects.Header.fixedFields.content
          }
        },
        Tag: {
          $visitor: _index1.default,
          fixedFields: {
            name: _apidomNsOpenapi.specificationObj.visitors.document.objects.Tag.fixedFields.name,
            description: _apidomNsOpenapi.specificationObj.visitors.document.objects.Tag.fixedFields.description,
            externalDocs: {
              $ref: '#/visitors/document/objects/ExternalDocumentation'
            }
          }
        },
        Reference: {
          $visitor: _index10.default,
          fixedFields: {
            $ref: _apidomNsOpenapi.specificationObj.visitors.document.objects.Reference.fixedFields.$ref,
            summary: {
              $ref: '#/visitors/value'
            },
            description: {
              $ref: '#/visitors/value'
            }
          }
        },
        JSONSchema: {
          $ref: '#/visitors/document/objects/Schema'
        },
        LinkDescription: {
          ...LinkDescriptionVisitor
        },
        Schema: {
          $visitor: _index13.default,
          fixedFields: {
            ...JSONSchemaVisitor.fixedFields,
            // core vocabulary
            $defs: _$defsVisitor.default,
            // applicator vocabulary
            allOf: _AllOfVisitor.default,
            anyOf: _AnyOfVisitor.default,
            oneOf: _OneOfVisitor.default,
            not: {
              $ref: '#/visitors/document/objects/Schema'
            },
            if: {
              $ref: '#/visitors/document/objects/Schema'
            },
            then: {
              $ref: '#/visitors/document/objects/Schema'
            },
            else: {
              $ref: '#/visitors/document/objects/Schema'
            },
            dependentSchemas: _DependentSchemasVisitor.default,
            prefixItems: _PrefixItemsVisitor.default,
            items: {
              $ref: '#/visitors/document/objects/Schema'
            },
            contains: {
              $ref: '#/visitors/document/objects/Schema'
            },
            properties: _PropertiesVisitor.default,
            patternProperties: _PatternPropertiesVisitor.default,
            additionalProperties: {
              $ref: '#/visitors/document/objects/Schema'
            },
            propertyNames: {
              $ref: '#/visitors/document/objects/Schema'
            },
            // unevaluated Locations vocabulary
            unevaluatedItems: {
              $ref: '#/visitors/document/objects/Schema'
            },
            unevaluatedProperties: {
              $ref: '#/visitors/document/objects/Schema'
            },
            // validation vocabulary
            // contents of String-Encoded Data vocabulary
            contentSchema: {
              $ref: '#/visitors/document/objects/Schema'
            },
            // OAS base vocabulary
            discriminator: {
              $ref: '#/visitors/document/objects/Discriminator'
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
        Discriminator: {
          $visitor: _index14.default,
          fixedFields: {
            propertyName: _apidomNsOpenapi.specificationObj.visitors.document.objects.Discriminator.fixedFields.propertyName,
            mapping: _apidomNsOpenapi.specificationObj.visitors.document.objects.Discriminator.fixedFields.mapping
          }
        },
        XML: {
          $visitor: _index15.default,
          fixedFields: {
            name: _apidomNsOpenapi.specificationObj.visitors.document.objects.XML.fixedFields.name,
            namespace: _apidomNsOpenapi.specificationObj.visitors.document.objects.XML.fixedFields.namespace,
            prefix: _apidomNsOpenapi.specificationObj.visitors.document.objects.XML.fixedFields.prefix,
            attribute: _apidomNsOpenapi.specificationObj.visitors.document.objects.XML.fixedFields.attribute,
            wrapped: _apidomNsOpenapi.specificationObj.visitors.document.objects.XML.fixedFields.wrapped
          }
        },
        SecurityScheme: {
          $visitor: _index26.default,
          fixedFields: {
            type: _apidomNsOpenapi.specificationObj.visitors.document.objects.SecurityScheme.fixedFields.type,
            description: _apidomNsOpenapi.specificationObj.visitors.document.objects.SecurityScheme.fixedFields.description,
            name: _apidomNsOpenapi.specificationObj.visitors.document.objects.SecurityScheme.fixedFields.name,
            in: _apidomNsOpenapi.specificationObj.visitors.document.objects.SecurityScheme.fixedFields.in,
            scheme: _apidomNsOpenapi.specificationObj.visitors.document.objects.SecurityScheme.fixedFields.scheme,
            bearerFormat: _apidomNsOpenapi.specificationObj.visitors.document.objects.SecurityScheme.fixedFields.bearerFormat,
            flows: {
              $ref: '#/visitors/document/objects/OAuthFlows'
            },
            openIdConnectUrl: _apidomNsOpenapi.specificationObj.visitors.document.objects.SecurityScheme.fixedFields.openIdConnectUrl
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
            authorizationUrl: _apidomNsOpenapi.specificationObj.visitors.document.objects.OAuthFlow.fixedFields.authorizationUrl,
            tokenUrl: _apidomNsOpenapi.specificationObj.visitors.document.objects.OAuthFlow.fixedFields.tokenUrl,
            refreshUrl: _apidomNsOpenapi.specificationObj.visitors.document.objects.OAuthFlow.fixedFields.refreshUrl,
            scopes: _apidomNsOpenapi.specificationObj.visitors.document.objects.OAuthFlow.fixedFields.scopes
          }
        },
        SecurityRequirement: {
          $visitor: _index9.default
        }
      },
      extension: {
        $visitor: _apidomNsOpenapi.specificationObj.visitors.document.extension.$visitor
      }
    }
  }
};
var _default = exports.default = specification;