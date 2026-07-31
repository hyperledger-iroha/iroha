import { specificationObj as JSONSchemaDraft4Specification } from '@swagger-api/apidom-ns-json-schema-draft-4';
import OpenApi3_0Visitor from "./visitors/open-api-3-0/index.mjs";
import OpenapiVisitor from "./visitors/open-api-3-0/OpenapiVisitor.mjs";
import SpecificationExtensionVisitor from "./visitors/SpecificationExtensionVisitor.mjs";
import InfoVisitor from "./visitors/open-api-3-0/info/index.mjs";
import InfoVersionVisitor from "./visitors/open-api-3-0/info/VersionVisitor.mjs";
import ContactVisitor from "./visitors/open-api-3-0/contact/index.mjs";
import LicenseVisitor from "./visitors/open-api-3-0/license/index.mjs";
import LinkVisitor from "./visitors/open-api-3-0/link/index.mjs";
import LinkOperationRefVisitor from "./visitors/open-api-3-0/link/OperationRefVisitor.mjs";
import LinkOperationIdVisitor from "./visitors/open-api-3-0/link/OperationIdVisitor.mjs";
import LinkParametersVisitor from "./visitors/open-api-3-0/link/ParametersVisitor.mjs";
import ServerVisitor from "./visitors/open-api-3-0/server/index.mjs";
import ServerUrlVisitor from "./visitors/open-api-3-0/server/UrlVisitor.mjs";
import ServersVisitor from "./visitors/open-api-3-0/ServersVisitor.mjs";
import ServerVariableVisitor from "./visitors/open-api-3-0/server-variable/index.mjs";
import ServerVariablesVisitor from "./visitors/open-api-3-0/server/VariablesVisitor.mjs";
import FallbackVisitor from "./visitors/FallbackVisitor.mjs";
import MediaTypeVisitor from "./visitors/open-api-3-0/media-type/index.mjs";
import MediaTypeSchemaVisitor from "./visitors/open-api-3-0/media-type/SchemaVisitor.mjs";
import MediaTypeExamplesVisitor from "./visitors/open-api-3-0/media-type/ExamplesVisitor.mjs";
import MediaTypeEncodingVisitor from "./visitors/open-api-3-0/media-type/EncodingVisitor.mjs";
import SecurityRequirementVisitor from "./visitors/open-api-3-0/security-requirement/index.mjs";
import SecurityVisitor from "./visitors/open-api-3-0/SecurityVisitor.mjs";
import ComponentsVisitor from "./visitors/open-api-3-0/components/index.mjs";
import TagVisitor from "./visitors/open-api-3-0/tag/index.mjs";
import ReferenceVisitor from "./visitors/open-api-3-0/reference/index.mjs";
import Reference$RefVisitor from "./visitors/open-api-3-0/reference/$RefVisitor.mjs";
import ParameterVisitor from "./visitors/open-api-3-0/parameter/index.mjs";
import ParameterSchemaVisitor from "./visitors/open-api-3-0/parameter/SchemaVisitor.mjs";
import HeaderVisitor from "./visitors/open-api-3-0/header/index.mjs";
import HeaderSchemaVisitor from "./visitors/open-api-3-0/header/SchemaVisitor.mjs";
import HeaderExamplesVisitor from "./visitors/open-api-3-0/header/ExamplesVisitor.mjs";
import HeaderContentVisitor from "./visitors/open-api-3-0/header/ContentVisitor.mjs";
import SchemaVisitor from "./visitors/open-api-3-0/schema/index.mjs";
import SchemaAllOfVisitor from "./visitors/open-api-3-0/schema/AllOfVisitor.mjs";
import SchemaAnyOfVisitor from "./visitors/open-api-3-0/schema/AnyOfVisitor.mjs";
import SchemaOneOfVisitor from "./visitors/open-api-3-0/schema/OneOfVisitor.mjs";
import SchemaItemsVisitor from "./visitors/open-api-3-0/schema/ItemsVisitor.mjs";
import SchemaPropertiesVisitor from "./visitors/open-api-3-0/schema/PropertiesVisitor.mjs";
import SchemaTypeVisitor from "./visitors/open-api-3-0/schema/TypeVisitor.mjs";
import SchemaOrReferenceVisitor from "./visitors/open-api-3-0/schema/SchemaOrReferenceVisitor.mjs";
import DiscriminatorVisitor from "./visitors/open-api-3-0/distriminator/index.mjs";
import DiscriminatorMappingVisitor from "./visitors/open-api-3-0/distriminator/MappingVisitor.mjs";
import XmlVisitor from "./visitors/open-api-3-0/xml/index.mjs";
import ParameterExamplesVisitor from "./visitors/open-api-3-0/parameter/ExamplesVisitor.mjs";
import ParameterContentVisitor from "./visitors/open-api-3-0/parameter/ContentVisitor.mjs";
import ComponentsSchemasVisitor from "./visitors/open-api-3-0/components/SchemasVisitor.mjs";
import ComponentsResponsesVisitor from "./visitors/open-api-3-0/components/ResponsesVisitor.mjs";
import ComponentsParametersVisitor from "./visitors/open-api-3-0/components/ParametersVisitor.mjs";
import ComponentsExamplesVisitor from "./visitors/open-api-3-0/components/ExamplesVisitor.mjs";
import ComponentsRequestBodiesVisitor from "./visitors/open-api-3-0/components/RequestBodiesVisitor.mjs";
import ComponentsHeadersVisitor from "./visitors/open-api-3-0/components/HeadersVisitor.mjs";
import ComponentsSecuritySchemesVisitor from "./visitors/open-api-3-0/components/SecuritySchemesVisitor.mjs";
import ComponentsLinksVisitor from "./visitors/open-api-3-0/components/LinksVisitor.mjs";
import ComponentsCallbacksVisitor from "./visitors/open-api-3-0/components/CallbacksVisitor.mjs";
import ExampleVisitor from "./visitors/open-api-3-0/example/index.mjs";
import ExampleExternalValueVisitor from "./visitors/open-api-3-0/example/ExternalValueVisitor.mjs";
import ExternalDocumentationVisitor from "./visitors/open-api-3-0/external-documentation/index.mjs";
import EncodingVisitor from "./visitors/open-api-3-0/encoding/index.mjs";
import EncodingHeadersVisitor from "./visitors/open-api-3-0/encoding/HeadersVisitor.mjs";
import PathsVisitor from "./visitors/open-api-3-0/paths/index.mjs";
import RequestBodyVisitor from "./visitors/open-api-3-0/request-body/index.mjs";
import RequestBodyContentVisitor from "./visitors/open-api-3-0/request-body/ContentVisitor.mjs";
import CallbackVisitor from "./visitors/open-api-3-0/callback/index.mjs";
import ResponseVisitor from "./visitors/open-api-3-0/response/index.mjs";
import ResponseHeadersVisitor from "./visitors/open-api-3-0/response/HeadersVisitor.mjs";
import ResponseContentVisitor from "./visitors/open-api-3-0/response/ContentVisitor.mjs";
import ResponseLinksVisitor from "./visitors/open-api-3-0/response/LinksVisitor.mjs";
import ResponsesVisitor from "./visitors/open-api-3-0/responses/index.mjs";
import ResponsesDefaultVisitor from "./visitors/open-api-3-0/responses/DefaultVisitor.mjs";
import OperationVisitor from "./visitors/open-api-3-0/operation/index.mjs";
import OperationTagsVisitor from "./visitors/open-api-3-0/operation/TagsVisitor.mjs";
import OperationParametersVisitor from "./visitors/open-api-3-0/operation/ParametersVisitor.mjs";
import OperationRequestBodyVisitor from "./visitors/open-api-3-0/operation/RequestBodyVisitor.mjs";
import OperationCallbacksVisitor from "./visitors/open-api-3-0/operation/CallbacksVisitor.mjs";
import OperationSecurityVisitor from "./visitors/open-api-3-0/operation/SecurityVisitor.mjs";
import OperationServersVisitor from "./visitors/open-api-3-0/operation/ServersVisitor.mjs";
import PathItemVisitor from "./visitors/open-api-3-0/path-item/index.mjs";
import PathItem$RefVisitor from "./visitors/open-api-3-0/path-item/$RefVisitor.mjs";
import PathItemServersVisitor from "./visitors/open-api-3-0/path-item/ServersVisitor.mjs";
import PathItemParametersVisitor from "./visitors/open-api-3-0/path-item/ParametersVisitor.mjs";
import SecuritySchemeVisitor from "./visitors/open-api-3-0/security-scheme/index.mjs";
import OAuthFlowsVisitor from "./visitors/open-api-3-0/oauth-flows/index.mjs";
import OAuthFlowVisitor from "./visitors/open-api-3-0/oauth-flow/index.mjs";
import OAuthFlowScopesVisitor from "./visitors/open-api-3-0/oauth-flow/ScopesVisitor.mjs";
import TagsVisitor from "./visitors/open-api-3-0/TagsVisitor.mjs";
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
} = JSONSchemaDraft4Specification.visitors.document.objects.JSONSchema;

/**
 * @public
 */
const specification = {
  visitors: {
    value: FallbackVisitor,
    document: {
      objects: {
        OpenApi: {
          $visitor: OpenApi3_0Visitor,
          fixedFields: {
            openapi: OpenapiVisitor,
            info: {
              $ref: '#/visitors/document/objects/Info'
            },
            servers: ServersVisitor,
            paths: {
              $ref: '#/visitors/document/objects/Paths'
            },
            components: {
              $ref: '#/visitors/document/objects/Components'
            },
            security: SecurityVisitor,
            tags: TagsVisitor,
            externalDocs: {
              $ref: '#/visitors/document/objects/ExternalDocumentation'
            }
          }
        },
        Info: {
          $visitor: InfoVisitor,
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
            version: InfoVersionVisitor
          }
        },
        Contact: {
          $visitor: ContactVisitor,
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
          $visitor: LicenseVisitor,
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
          $visitor: ServerVisitor,
          fixedFields: {
            url: ServerUrlVisitor,
            description: {
              $ref: '#/visitors/value'
            },
            variables: ServerVariablesVisitor
          }
        },
        ServerVariable: {
          $visitor: ServerVariableVisitor,
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
          $visitor: ComponentsVisitor,
          fixedFields: {
            schemas: ComponentsSchemasVisitor,
            responses: ComponentsResponsesVisitor,
            parameters: ComponentsParametersVisitor,
            examples: ComponentsExamplesVisitor,
            requestBodies: ComponentsRequestBodiesVisitor,
            headers: ComponentsHeadersVisitor,
            securitySchemes: ComponentsSecuritySchemesVisitor,
            links: ComponentsLinksVisitor,
            callbacks: ComponentsCallbacksVisitor
          }
        },
        Paths: {
          $visitor: PathsVisitor
        },
        PathItem: {
          $visitor: PathItemVisitor,
          fixedFields: {
            $ref: PathItem$RefVisitor,
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
            servers: PathItemServersVisitor,
            parameters: PathItemParametersVisitor
          }
        },
        Operation: {
          $visitor: OperationVisitor,
          fixedFields: {
            tags: OperationTagsVisitor,
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
            parameters: OperationParametersVisitor,
            requestBody: OperationRequestBodyVisitor,
            responses: {
              $ref: '#/visitors/document/objects/Responses'
            },
            callbacks: OperationCallbacksVisitor,
            deprecated: {
              $ref: '#/visitors/value'
            },
            security: OperationSecurityVisitor,
            servers: OperationServersVisitor
          }
        },
        ExternalDocumentation: {
          $visitor: ExternalDocumentationVisitor,
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
          $visitor: ParameterVisitor,
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
            schema: ParameterSchemaVisitor,
            example: {
              $ref: '#/visitors/value'
            },
            examples: ParameterExamplesVisitor,
            content: ParameterContentVisitor
          }
        },
        RequestBody: {
          $visitor: RequestBodyVisitor,
          fixedFields: {
            description: {
              $ref: '#/visitors/value'
            },
            content: RequestBodyContentVisitor,
            required: {
              $ref: '#/visitors/value'
            }
          }
        },
        MediaType: {
          $visitor: MediaTypeVisitor,
          fixedFields: {
            schema: MediaTypeSchemaVisitor,
            example: {
              $ref: '#/visitors/value'
            },
            examples: MediaTypeExamplesVisitor,
            encoding: MediaTypeEncodingVisitor
          }
        },
        Encoding: {
          $visitor: EncodingVisitor,
          fixedFields: {
            contentType: {
              $ref: '#/visitors/value'
            },
            headers: EncodingHeadersVisitor,
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
          $visitor: ResponsesVisitor,
          fixedFields: {
            default: ResponsesDefaultVisitor
          }
        },
        Response: {
          $visitor: ResponseVisitor,
          fixedFields: {
            description: {
              $ref: '#/visitors/value'
            },
            headers: ResponseHeadersVisitor,
            content: ResponseContentVisitor,
            links: ResponseLinksVisitor
          }
        },
        Callback: {
          $visitor: CallbackVisitor
        },
        Example: {
          $visitor: ExampleVisitor,
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
            externalValue: ExampleExternalValueVisitor
          }
        },
        Link: {
          $visitor: LinkVisitor,
          fixedFields: {
            operationRef: LinkOperationRefVisitor,
            operationId: LinkOperationIdVisitor,
            parameters: LinkParametersVisitor,
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
          $visitor: HeaderVisitor,
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
            schema: HeaderSchemaVisitor,
            example: {
              $ref: '#/visitors/value'
            },
            examples: HeaderExamplesVisitor,
            content: HeaderContentVisitor
          }
        },
        Tag: {
          $visitor: TagVisitor,
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
          $visitor: ReferenceVisitor,
          fixedFields: {
            $ref: Reference$RefVisitor
          }
        },
        JSONSchema: {
          $ref: '#/visitors/document/objects/Schema'
        },
        JSONReference: {
          $ref: '#/visitors/document/objects/Reference'
        },
        Schema: {
          $visitor: SchemaVisitor,
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
            type: SchemaTypeVisitor,
            allOf: SchemaAllOfVisitor,
            anyOf: SchemaAnyOfVisitor,
            oneOf: SchemaOneOfVisitor,
            not: SchemaOrReferenceVisitor,
            items: SchemaItemsVisitor,
            properties: SchemaPropertiesVisitor,
            additionalProperties: SchemaOrReferenceVisitor,
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
          $visitor: DiscriminatorVisitor,
          fixedFields: {
            propertyName: {
              $ref: '#/visitors/value'
            },
            mapping: DiscriminatorMappingVisitor
          }
        },
        XML: {
          $visitor: XmlVisitor,
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
          $visitor: SecuritySchemeVisitor,
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
            authorizationUrl: {
              $ref: '#/visitors/value'
            },
            tokenUrl: {
              $ref: '#/visitors/value'
            },
            refreshUrl: {
              $ref: '#/visitors/value'
            },
            scopes: OAuthFlowScopesVisitor
          }
        },
        SecurityRequirement: {
          $visitor: SecurityRequirementVisitor
        }
      },
      extension: {
        $visitor: SpecificationExtensionVisitor
      }
    }
  }
};
export default specification;