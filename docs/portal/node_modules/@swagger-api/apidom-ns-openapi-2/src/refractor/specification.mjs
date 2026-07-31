import { specificationObj as JSONSchemaDraft4Specification } from '@swagger-api/apidom-ns-json-schema-draft-4';
import FallbackVisitor from "./visitors/FallbackVisitor.mjs";
import SwaggerVisitor from "./visitors/open-api-2/index.mjs";
import SwaggerSwaggerVisitor from "./visitors/open-api-2/SwaggerVisitor.mjs";
import SwaggerHostVisitor from "./visitors/open-api-2/HostVisitor.mjs";
import SwaggerBasePathVisitor from "./visitors/open-api-2/BasePathVisitor.mjs";
import SwaggerSchemesVisitor from "./visitors/open-api-2/SchemesVisitor.mjs";
import SwaggerConsumesVisitor from "./visitors/open-api-2/ConsumesVisitor.mjs";
import SwaggerProducesVisitor from "./visitors/open-api-2/ProducesVisitor.mjs";
import SwaggerSecurityVisitor from "./visitors/open-api-2/SecurityVisitor.mjs";
import SwaggerTagsVisitor from "./visitors/open-api-2/TagsVisitor.mjs";
import InfoVisitor from "./visitors/open-api-2/info/index.mjs";
import InfoVersionVisitor from "./visitors/open-api-2/info/VersionVisitor.mjs";
import ContactVisitor from "./visitors/open-api-2/contact/index.mjs";
import LicenseVisitor from "./visitors/open-api-2/license/index.mjs";
import PathsVisitor from "./visitors/open-api-2/paths/index.mjs";
import PathItemVisitor from "./visitors/open-api-2/path-item/index.mjs";
import PathItem$RefVisitor from "./visitors/open-api-2/path-item/$RefVisitor.mjs";
import PathItemParametersVisitor from "./visitors/open-api-2/path-item/ParametersVisitor.mjs";
import OperationVisitor from "./visitors/open-api-2/operation/index.mjs";
import OperationTagsVisitor from "./visitors/open-api-2/operation/TagsVisitor.mjs";
import OperationConsumesVisitor from "./visitors/open-api-2/operation/ConsumesVisitor.mjs";
import OperationProducesVisitor from "./visitors/open-api-2/operation/ProducesVisitor.mjs";
import OperationParametersVisitor from "./visitors/open-api-2/operation/ParametersVisitor.mjs";
import OperationSchemesVisitor from "./visitors/open-api-2/operation/SchemesVisitor.mjs";
import OperationSecurityVisitor from "./visitors/open-api-2/operation/SecurityVisitor.mjs";
import ExternalDocumentationElement from "./visitors/open-api-2/external-documentation/index.mjs";
import ParameterVisitor from "./visitors/open-api-2/parameter/index.mjs";
import ItemsVisitor from "./visitors/open-api-2/items/index.mjs";
import ResponsesVisitor from "./visitors/open-api-2/responses/index.mjs";
import ResponsesDefaultVisitor from "./visitors/open-api-2/responses/DefaultVisitor.mjs";
import ResponseVisitor from "./visitors/open-api-2/response/index.mjs";
import HeadersVisitor from "./visitors/open-api-2/headers/index.mjs";
import ExampleVisitor from "./visitors/open-api-2/example/index.mjs";
import HeaderVisitor from "./visitors/open-api-2/header/index.mjs";
import TagVisitor from "./visitors/open-api-2/tag/index.mjs";
import ReferenceVisitor from "./visitors/open-api-2/reference/index.mjs";
import Reference$RefVisitor from "./visitors/open-api-2/reference/$RefVisitor.mjs";
import SchemaVisitor from "./visitors/open-api-2/schema/index.mjs";
import SchemaAllOfVisitor from "./visitors/open-api-2/schema/AllOfVisitor.mjs";
import SchemaItemsVisitor from "./visitors/open-api-2/schema/ItemsVisitor.mjs";
import SchemaPropertiesVisitor from "./visitors/open-api-2/schema/PropertiesVisitor.mjs";
import SchemaOrJSONReferenceVisitor from "./visitors/open-api-2/schema/SchemaOrJSONReferenceVisitor.mjs";
import XmlVisitor from "./visitors/open-api-2/xml/index.mjs";
import DefinitionsVisitor from "./visitors/open-api-2/definitions/index.mjs";
import ResponsesDefinitionsVisitor from "./visitors/open-api-2/responses-definitions/index.mjs";
import ParametersDefinitionsVisitor from "./visitors/open-api-2/parameters-definitions/index.mjs";
import SecurityDefinitionsVisitor from "./visitors/open-api-2/security-definitions/index.mjs";
import SecuritySchemeVisitor from "./visitors/open-api-2/security-scheme/index.mjs";
import ScopesVisitor from "./visitors/open-api-2/scopes/index.mjs";
import SecurityRequirementVisitor from "./visitors/open-api-2/security-requirement/index.mjs";
import SpecificationExtensionVisitor from "./visitors/SpecificationExtensionVisitor.mjs";
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
        // JSON Schema Draft 4/5 specific visitors
        JSONReference: JSONSchemaDraft4Specification.visitors.document.objects.JSONReference,
        JSONSchema: {
          $ref: '#/visitors/document/objects/Schema'
        },
        // OpenAPI 2 specific visitors
        Swagger: {
          $visitor: SwaggerVisitor,
          fixedFields: {
            swagger: SwaggerSwaggerVisitor,
            info: {
              $ref: '#/visitors/document/objects/Info'
            },
            host: SwaggerHostVisitor,
            basePath: SwaggerBasePathVisitor,
            schemes: SwaggerSchemesVisitor,
            consumes: SwaggerConsumesVisitor,
            produces: SwaggerProducesVisitor,
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
            security: SwaggerSecurityVisitor,
            tags: SwaggerTagsVisitor,
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
        Paths: {
          $visitor: PathsVisitor
        },
        PathItem: {
          $visitor: PathItemVisitor,
          fixedFields: {
            $ref: PathItem$RefVisitor,
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
            consumes: OperationConsumesVisitor,
            produces: OperationProducesVisitor,
            parameters: OperationParametersVisitor,
            responses: {
              $ref: '#/visitors/document/objects/Responses'
            },
            schemes: OperationSchemesVisitor,
            deprecated: {
              $ref: '#/visitors/value'
            },
            security: OperationSecurityVisitor
          }
        },
        ExternalDocumentation: {
          $visitor: ExternalDocumentationElement,
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
            description: jsonSchemaFixedFields.description,
            required: jsonSchemaFixedFields.required,
            schema: SchemaOrJSONReferenceVisitor,
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
          $visitor: ItemsVisitor,
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
            schema: SchemaOrJSONReferenceVisitor,
            headers: {
              $ref: '#/visitors/document/objects/Headers'
            },
            examples: {
              $ref: '#/visitors/document/objects/Example'
            }
          }
        },
        Headers: {
          $visitor: HeadersVisitor
        },
        Example: {
          $visitor: ExampleVisitor
        },
        Header: {
          $visitor: HeaderVisitor,
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
        Schema: {
          $visitor: SchemaVisitor,
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
            items: SchemaItemsVisitor,
            allOf: SchemaAllOfVisitor,
            properties: SchemaPropertiesVisitor,
            additionalProperties: SchemaOrJSONReferenceVisitor,
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
        Definitions: {
          $visitor: DefinitionsVisitor
        },
        ParametersDefinitions: {
          $visitor: ParametersDefinitionsVisitor
        },
        ResponsesDefinitions: {
          $visitor: ResponsesDefinitionsVisitor
        },
        SecurityDefinitions: {
          $visitor: SecurityDefinitionsVisitor
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
          $visitor: ScopesVisitor
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