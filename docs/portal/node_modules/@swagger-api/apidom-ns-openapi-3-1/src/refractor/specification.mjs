import { specificationObj as OpenApi3_0Specification } from '@swagger-api/apidom-ns-openapi-3-0';
import { specificationObj as JSONSchema202012Specification } from '@swagger-api/apidom-ns-json-schema-2020-12';
import OpenApi3_1Visitor from "./visitors/open-api-3-1/index.mjs";
import InfoVisitor from "./visitors/open-api-3-1/info/index.mjs";
import ContactVisitor from "./visitors/open-api-3-1/contact/index.mjs";
import LicenseVisitor from "./visitors/open-api-3-1/license/index.mjs";
import LinkVisitor from "./visitors/open-api-3-1/link/index.mjs";
import JsonSchemaDialectVisitor from "./visitors/open-api-3-1/JsonSchemaDialectVisitor.mjs";
import ServerVisitor from "./visitors/open-api-3-1/server/index.mjs";
import ServerVariableVisitor from "./visitors/open-api-3-1/server-variable/index.mjs";
import MediaTypeVisitor from "./visitors/open-api-3-1/media-type/index.mjs";
import SecurityRequirementVisitor from "./visitors/open-api-3-1/security-requirement/index.mjs";
import ComponentsVisitor from "./visitors/open-api-3-1/components/index.mjs";
import TagVisitor from "./visitors/open-api-3-1/tag/index.mjs";
import ReferenceVisitor from "./visitors/open-api-3-1/reference/index.mjs";
import ParameterVisitor from "./visitors/open-api-3-1/parameter/index.mjs";
import HeaderVisitor from "./visitors/open-api-3-1/header/index.mjs";
import SchemaVisitor from "./visitors/open-api-3-1/schema/index.mjs";
import Schema$defsVisitor from "./visitors/open-api-3-1/schema/$defsVisitor.mjs";
import SchemaAllOfVisitor from "./visitors/open-api-3-1/schema/AllOfVisitor.mjs";
import SchemaAnyOfVisitor from "./visitors/open-api-3-1/schema/AnyOfVisitor.mjs";
import SchemaOneOfVisitor from "./visitors/open-api-3-1/schema/OneOfVisitor.mjs";
import SchemaDependantSchemasVisitor from "./visitors/open-api-3-1/schema/DependentSchemasVisitor.mjs";
import SchemaPrefixItemsVisitor from "./visitors/open-api-3-1/schema/PrefixItemsVisitor.mjs";
import SchemaPropertiesVisitor from "./visitors/open-api-3-1/schema/PropertiesVisitor.mjs";
import SchemaPatternPropertiesVisitor from "./visitors/open-api-3-1/schema/PatternPropertiesVisitor.mjs";
import DiscriminatorVisitor from "./visitors/open-api-3-1/distriminator/index.mjs";
import XmlVisitor from "./visitors/open-api-3-1/xml/index.mjs";
import ComponentsSchemasVisitor from "./visitors/open-api-3-1/components/SchemasVisitor.mjs";
import ComponentsPathItemsVisitor from "./visitors/open-api-3-1/components/PathItemsVisitor.mjs";
import ExampleVisitor from "./visitors/open-api-3-1/example/index.mjs";
import ExternalDocumentationVisitor from "./visitors/open-api-3-1/external-documentation/index.mjs";
import EncodingVisitor from "./visitors/open-api-3-1/encoding/index.mjs";
import PathsVisitor from "./visitors/open-api-3-1/paths/index.mjs";
import RequestBodyVisitor from "./visitors/open-api-3-1/request-body/index.mjs";
import CallbackVisitor from "./visitors/open-api-3-1/callback/index.mjs";
import ResponseVisitor from "./visitors/open-api-3-1/response/index.mjs";
import ResponsesVisitor from "./visitors/open-api-3-1/responses/index.mjs";
import OperationVisitor from "./visitors/open-api-3-1/operation/index.mjs";
import PathItemVisitor from "./visitors/open-api-3-1/path-item/index.mjs";
import SecuritySchemeVisitor from "./visitors/open-api-3-1/security-scheme/index.mjs";
import OAuthFlowsVisitor from "./visitors/open-api-3-1/oauth-flows/index.mjs";
import OAuthFlowVisitor from "./visitors/open-api-3-1/oauth-flow/index.mjs";
import WebhooksVisitor from "./visitors/open-api-3-1/WebhooksVisitor.mjs";
const {
  JSONSchema: JSONSchemaVisitor,
  LinkDescription: LinkDescriptionVisitor
} = JSONSchema202012Specification.visitors.document.objects;

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
    value: OpenApi3_0Specification.visitors.value,
    document: {
      objects: {
        OpenApi: {
          $visitor: OpenApi3_1Visitor,
          fixedFields: {
            openapi: OpenApi3_0Specification.visitors.document.objects.OpenApi.fixedFields.openapi,
            info: {
              $ref: '#/visitors/document/objects/Info'
            },
            jsonSchemaDialect: JsonSchemaDialectVisitor,
            servers: OpenApi3_0Specification.visitors.document.objects.OpenApi.fixedFields.servers,
            paths: {
              $ref: '#/visitors/document/objects/Paths'
            },
            webhooks: WebhooksVisitor,
            components: {
              $ref: '#/visitors/document/objects/Components'
            },
            security: OpenApi3_0Specification.visitors.document.objects.OpenApi.fixedFields.security,
            tags: OpenApi3_0Specification.visitors.document.objects.OpenApi.fixedFields.tags,
            externalDocs: {
              $ref: '#/visitors/document/objects/ExternalDocumentation'
            }
          }
        },
        Info: {
          $visitor: InfoVisitor,
          fixedFields: {
            title: OpenApi3_0Specification.visitors.document.objects.Info.fixedFields.title,
            description: OpenApi3_0Specification.visitors.document.objects.Info.fixedFields.description,
            summary: {
              $ref: '#/visitors/value'
            },
            termsOfService: OpenApi3_0Specification.visitors.document.objects.Info.fixedFields.termsOfService,
            contact: {
              $ref: '#/visitors/document/objects/Contact'
            },
            license: {
              $ref: '#/visitors/document/objects/License'
            },
            version: OpenApi3_0Specification.visitors.document.objects.Info.fixedFields.version
          }
        },
        Contact: {
          $visitor: ContactVisitor,
          fixedFields: {
            name: OpenApi3_0Specification.visitors.document.objects.Contact.fixedFields.name,
            url: OpenApi3_0Specification.visitors.document.objects.Contact.fixedFields.url,
            email: OpenApi3_0Specification.visitors.document.objects.Contact.fixedFields.email
          }
        },
        License: {
          $visitor: LicenseVisitor,
          fixedFields: {
            name: OpenApi3_0Specification.visitors.document.objects.License.fixedFields.name,
            identifier: {
              $ref: '#/visitors/value'
            },
            url: OpenApi3_0Specification.visitors.document.objects.License.fixedFields.url
          }
        },
        Server: {
          $visitor: ServerVisitor,
          fixedFields: {
            url: OpenApi3_0Specification.visitors.document.objects.Server.fixedFields.url,
            description: OpenApi3_0Specification.visitors.document.objects.Server.fixedFields.description,
            variables: OpenApi3_0Specification.visitors.document.objects.Server.fixedFields.variables
          }
        },
        ServerVariable: {
          $visitor: ServerVariableVisitor,
          fixedFields: {
            enum: OpenApi3_0Specification.visitors.document.objects.ServerVariable.fixedFields.enum,
            default: OpenApi3_0Specification.visitors.document.objects.ServerVariable.fixedFields.default,
            description: OpenApi3_0Specification.visitors.document.objects.ServerVariable.fixedFields.description
          }
        },
        Components: {
          $visitor: ComponentsVisitor,
          fixedFields: {
            schemas: ComponentsSchemasVisitor,
            responses: OpenApi3_0Specification.visitors.document.objects.Components.fixedFields.responses,
            parameters: OpenApi3_0Specification.visitors.document.objects.Components.fixedFields.parameters,
            examples: OpenApi3_0Specification.visitors.document.objects.Components.fixedFields.examples,
            requestBodies: OpenApi3_0Specification.visitors.document.objects.Components.fixedFields.requestBodies,
            headers: OpenApi3_0Specification.visitors.document.objects.Components.fixedFields.headers,
            securitySchemes: OpenApi3_0Specification.visitors.document.objects.Components.fixedFields.securitySchemes,
            links: OpenApi3_0Specification.visitors.document.objects.Components.fixedFields.links,
            callbacks: OpenApi3_0Specification.visitors.document.objects.Components.fixedFields.callbacks,
            pathItems: ComponentsPathItemsVisitor
          }
        },
        Paths: {
          $visitor: PathsVisitor
        },
        PathItem: {
          $visitor: PathItemVisitor,
          fixedFields: {
            $ref: OpenApi3_0Specification.visitors.document.objects.PathItem.fixedFields.$ref,
            summary: OpenApi3_0Specification.visitors.document.objects.PathItem.fixedFields.summary,
            description: OpenApi3_0Specification.visitors.document.objects.PathItem.fixedFields.description,
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
            servers: OpenApi3_0Specification.visitors.document.objects.PathItem.fixedFields.servers,
            parameters: OpenApi3_0Specification.visitors.document.objects.PathItem.fixedFields.parameters
          }
        },
        Operation: {
          $visitor: OperationVisitor,
          fixedFields: {
            tags: OpenApi3_0Specification.visitors.document.objects.Operation.fixedFields.tags,
            summary: OpenApi3_0Specification.visitors.document.objects.Operation.fixedFields.summary,
            description: OpenApi3_0Specification.visitors.document.objects.Operation.fixedFields.description,
            externalDocs: {
              $ref: '#/visitors/document/objects/ExternalDocumentation'
            },
            operationId: OpenApi3_0Specification.visitors.document.objects.Operation.fixedFields.operationId,
            parameters: OpenApi3_0Specification.visitors.document.objects.Operation.fixedFields.parameters,
            requestBody: OpenApi3_0Specification.visitors.document.objects.Operation.fixedFields.requestBody,
            responses: {
              $ref: '#/visitors/document/objects/Responses'
            },
            callbacks: OpenApi3_0Specification.visitors.document.objects.Operation.fixedFields.callbacks,
            deprecated: OpenApi3_0Specification.visitors.document.objects.Operation.fixedFields.deprecated,
            security: OpenApi3_0Specification.visitors.document.objects.Operation.fixedFields.security,
            servers: OpenApi3_0Specification.visitors.document.objects.Operation.fixedFields.servers
          }
        },
        ExternalDocumentation: {
          $visitor: ExternalDocumentationVisitor,
          fixedFields: {
            description: OpenApi3_0Specification.visitors.document.objects.ExternalDocumentation.fixedFields.description,
            url: OpenApi3_0Specification.visitors.document.objects.ExternalDocumentation.fixedFields.url
          }
        },
        Parameter: {
          $visitor: ParameterVisitor,
          fixedFields: {
            name: OpenApi3_0Specification.visitors.document.objects.Parameter.fixedFields.name,
            in: OpenApi3_0Specification.visitors.document.objects.Parameter.fixedFields.in,
            description: OpenApi3_0Specification.visitors.document.objects.Parameter.fixedFields.description,
            required: OpenApi3_0Specification.visitors.document.objects.Parameter.fixedFields.required,
            deprecated: OpenApi3_0Specification.visitors.document.objects.Parameter.fixedFields.deprecated,
            allowEmptyValue: OpenApi3_0Specification.visitors.document.objects.Parameter.fixedFields.allowEmptyValue,
            style: OpenApi3_0Specification.visitors.document.objects.Parameter.fixedFields.style,
            explode: OpenApi3_0Specification.visitors.document.objects.Parameter.fixedFields.explode,
            allowReserved: OpenApi3_0Specification.visitors.document.objects.Parameter.fixedFields.allowReserved,
            schema: {
              $ref: '#/visitors/document/objects/Schema'
            },
            example: OpenApi3_0Specification.visitors.document.objects.Parameter.fixedFields.example,
            examples: OpenApi3_0Specification.visitors.document.objects.Parameter.fixedFields.examples,
            content: OpenApi3_0Specification.visitors.document.objects.Parameter.fixedFields.content
          }
        },
        RequestBody: {
          $visitor: RequestBodyVisitor,
          fixedFields: {
            description: OpenApi3_0Specification.visitors.document.objects.RequestBody.fixedFields.description,
            content: OpenApi3_0Specification.visitors.document.objects.RequestBody.fixedFields.content,
            required: OpenApi3_0Specification.visitors.document.objects.RequestBody.fixedFields.required
          }
        },
        MediaType: {
          $visitor: MediaTypeVisitor,
          fixedFields: {
            schema: {
              $ref: '#/visitors/document/objects/Schema'
            },
            example: OpenApi3_0Specification.visitors.document.objects.MediaType.fixedFields.example,
            examples: OpenApi3_0Specification.visitors.document.objects.MediaType.fixedFields.examples,
            encoding: OpenApi3_0Specification.visitors.document.objects.MediaType.fixedFields.encoding
          }
        },
        Encoding: {
          $visitor: EncodingVisitor,
          fixedFields: {
            contentType: OpenApi3_0Specification.visitors.document.objects.Encoding.fixedFields.contentType,
            headers: OpenApi3_0Specification.visitors.document.objects.Encoding.fixedFields.headers,
            style: OpenApi3_0Specification.visitors.document.objects.Encoding.fixedFields.style,
            explode: OpenApi3_0Specification.visitors.document.objects.Encoding.fixedFields.explode,
            allowReserved: OpenApi3_0Specification.visitors.document.objects.Encoding.fixedFields.allowReserved
          }
        },
        Responses: {
          $visitor: ResponsesVisitor,
          fixedFields: {
            default: OpenApi3_0Specification.visitors.document.objects.Responses.fixedFields.default
          }
        },
        Response: {
          $visitor: ResponseVisitor,
          fixedFields: {
            description: OpenApi3_0Specification.visitors.document.objects.Response.fixedFields.description,
            headers: OpenApi3_0Specification.visitors.document.objects.Response.fixedFields.headers,
            content: OpenApi3_0Specification.visitors.document.objects.Response.fixedFields.content,
            links: OpenApi3_0Specification.visitors.document.objects.Response.fixedFields.links
          }
        },
        Callback: {
          $visitor: CallbackVisitor
        },
        Example: {
          $visitor: ExampleVisitor,
          fixedFields: {
            summary: OpenApi3_0Specification.visitors.document.objects.Example.fixedFields.summary,
            description: OpenApi3_0Specification.visitors.document.objects.Example.fixedFields.description,
            value: OpenApi3_0Specification.visitors.document.objects.Example.fixedFields.value,
            externalValue: OpenApi3_0Specification.visitors.document.objects.Example.fixedFields.externalValue
          }
        },
        Link: {
          $visitor: LinkVisitor,
          fixedFields: {
            operationRef: OpenApi3_0Specification.visitors.document.objects.Link.fixedFields.operationRef,
            operationId: OpenApi3_0Specification.visitors.document.objects.Link.fixedFields.operationId,
            parameters: OpenApi3_0Specification.visitors.document.objects.Link.fixedFields.parameters,
            requestBody: OpenApi3_0Specification.visitors.document.objects.Link.fixedFields.requestBody,
            description: OpenApi3_0Specification.visitors.document.objects.Link.fixedFields.description,
            server: {
              $ref: '#/visitors/document/objects/Server'
            }
          }
        },
        Header: {
          $visitor: HeaderVisitor,
          fixedFields: {
            description: OpenApi3_0Specification.visitors.document.objects.Header.fixedFields.description,
            required: OpenApi3_0Specification.visitors.document.objects.Header.fixedFields.required,
            deprecated: OpenApi3_0Specification.visitors.document.objects.Header.fixedFields.deprecated,
            allowEmptyValue: OpenApi3_0Specification.visitors.document.objects.Header.fixedFields.allowEmptyValue,
            style: OpenApi3_0Specification.visitors.document.objects.Header.fixedFields.style,
            explode: OpenApi3_0Specification.visitors.document.objects.Header.fixedFields.explode,
            allowReserved: OpenApi3_0Specification.visitors.document.objects.Header.fixedFields.allowReserved,
            schema: {
              $ref: '#/visitors/document/objects/Schema'
            },
            example: OpenApi3_0Specification.visitors.document.objects.Header.fixedFields.example,
            examples: OpenApi3_0Specification.visitors.document.objects.Header.fixedFields.examples,
            content: OpenApi3_0Specification.visitors.document.objects.Header.fixedFields.content
          }
        },
        Tag: {
          $visitor: TagVisitor,
          fixedFields: {
            name: OpenApi3_0Specification.visitors.document.objects.Tag.fixedFields.name,
            description: OpenApi3_0Specification.visitors.document.objects.Tag.fixedFields.description,
            externalDocs: {
              $ref: '#/visitors/document/objects/ExternalDocumentation'
            }
          }
        },
        Reference: {
          $visitor: ReferenceVisitor,
          fixedFields: {
            $ref: OpenApi3_0Specification.visitors.document.objects.Reference.fixedFields.$ref,
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
          $visitor: SchemaVisitor,
          fixedFields: {
            ...JSONSchemaVisitor.fixedFields,
            // core vocabulary
            $defs: Schema$defsVisitor,
            // applicator vocabulary
            allOf: SchemaAllOfVisitor,
            anyOf: SchemaAnyOfVisitor,
            oneOf: SchemaOneOfVisitor,
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
            dependentSchemas: SchemaDependantSchemasVisitor,
            prefixItems: SchemaPrefixItemsVisitor,
            items: {
              $ref: '#/visitors/document/objects/Schema'
            },
            contains: {
              $ref: '#/visitors/document/objects/Schema'
            },
            properties: SchemaPropertiesVisitor,
            patternProperties: SchemaPatternPropertiesVisitor,
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
          $visitor: DiscriminatorVisitor,
          fixedFields: {
            propertyName: OpenApi3_0Specification.visitors.document.objects.Discriminator.fixedFields.propertyName,
            mapping: OpenApi3_0Specification.visitors.document.objects.Discriminator.fixedFields.mapping
          }
        },
        XML: {
          $visitor: XmlVisitor,
          fixedFields: {
            name: OpenApi3_0Specification.visitors.document.objects.XML.fixedFields.name,
            namespace: OpenApi3_0Specification.visitors.document.objects.XML.fixedFields.namespace,
            prefix: OpenApi3_0Specification.visitors.document.objects.XML.fixedFields.prefix,
            attribute: OpenApi3_0Specification.visitors.document.objects.XML.fixedFields.attribute,
            wrapped: OpenApi3_0Specification.visitors.document.objects.XML.fixedFields.wrapped
          }
        },
        SecurityScheme: {
          $visitor: SecuritySchemeVisitor,
          fixedFields: {
            type: OpenApi3_0Specification.visitors.document.objects.SecurityScheme.fixedFields.type,
            description: OpenApi3_0Specification.visitors.document.objects.SecurityScheme.fixedFields.description,
            name: OpenApi3_0Specification.visitors.document.objects.SecurityScheme.fixedFields.name,
            in: OpenApi3_0Specification.visitors.document.objects.SecurityScheme.fixedFields.in,
            scheme: OpenApi3_0Specification.visitors.document.objects.SecurityScheme.fixedFields.scheme,
            bearerFormat: OpenApi3_0Specification.visitors.document.objects.SecurityScheme.fixedFields.bearerFormat,
            flows: {
              $ref: '#/visitors/document/objects/OAuthFlows'
            },
            openIdConnectUrl: OpenApi3_0Specification.visitors.document.objects.SecurityScheme.fixedFields.openIdConnectUrl
          }
        },
        OAuthFlows: {
          $visitor: OAuthFlowsVisitor,
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
          $visitor: OAuthFlowVisitor,
          fixedFields: {
            authorizationUrl: OpenApi3_0Specification.visitors.document.objects.OAuthFlow.fixedFields.authorizationUrl,
            tokenUrl: OpenApi3_0Specification.visitors.document.objects.OAuthFlow.fixedFields.tokenUrl,
            refreshUrl: OpenApi3_0Specification.visitors.document.objects.OAuthFlow.fixedFields.refreshUrl,
            scopes: OpenApi3_0Specification.visitors.document.objects.OAuthFlow.fixedFields.scopes
          }
        },
        SecurityRequirement: {
          $visitor: SecurityRequirementVisitor
        }
      },
      extension: {
        $visitor: OpenApi3_0Specification.visitors.document.extension.$visitor
      }
    }
  }
};
export default specification;