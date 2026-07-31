import FallbackVisitor from "./visitors/FallbackVisitor.mjs";
import JSONSchemaVisitor from "./visitors/json-schema/index.mjs";
import JSONSchemaItemsVisitor from "./visitors/json-schema/ItemsVisitor.mjs";
import JSONSchemaRequiredVisitor from "./visitors/json-schema/RequiredVisitor.mjs";
import JSONSchemaPropertiesVisitor from "./visitors/json-schema/PropertiesVisitor.mjs";
import JSONSchemaPatternPropertiesVisitor from "./visitors/json-schema/PatternPropertiesVisitor.mjs";
import JSONSchemaDependenciesVisitor from "./visitors/json-schema/DependenciesVisitor.mjs";
import JSONSchemaEnumVisitor from "./visitors/json-schema/EnumVisitor.mjs";
import JSONSchemaTypeVisitor from "./visitors/json-schema/TypeVisitor.mjs";
import JSONSchemaAllOfVisitor from "./visitors/json-schema/AllOfVisitor.mjs";
import JSONSchemaAnyOfVisitor from "./visitors/json-schema/AnyOfVisitor.mjs";
import JSONSchemaOneOfVisitor from "./visitors/json-schema/OneOfVisitor.mjs";
import JSONSchemaDefinitionsVisitor from "./visitors/json-schema/DefinitionsVisitor.mjs";
import JSONSchemaLinksVisitor from "./visitors/json-schema/LinksVisitor.mjs";
import JSONReferenceVisitor from "./visitors/json-schema/json-reference/index.mjs";
import JSONReference$RefVisitor from "./visitors/json-schema/json-reference/$RefVisitor.mjs";
import JSONSchemaOrJSONReferenceVisitor from "./visitors/json-schema/JSONSchemaOrJSONReferenceVisitor.mjs";
import MediaVisitor from "./visitors/json-schema/media/index.mjs";
import LinkDescriptionVisitor from "./visitors/json-schema/link-description/index.mjs";
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
    value: FallbackVisitor,
    JSONSchemaOrJSONReferenceVisitor,
    document: {
      objects: {
        JSONSchema: {
          $visitor: JSONSchemaVisitor,
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
            additionalItems: JSONSchemaOrJSONReferenceVisitor,
            items: JSONSchemaItemsVisitor,
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
            required: JSONSchemaRequiredVisitor,
            properties: JSONSchemaPropertiesVisitor,
            additionalProperties: JSONSchemaOrJSONReferenceVisitor,
            patternProperties: JSONSchemaPatternPropertiesVisitor,
            dependencies: JSONSchemaDependenciesVisitor,
            // validation keywords for any instance type
            enum: JSONSchemaEnumVisitor,
            type: JSONSchemaTypeVisitor,
            allOf: JSONSchemaAllOfVisitor,
            anyOf: JSONSchemaAnyOfVisitor,
            oneOf: JSONSchemaOneOfVisitor,
            not: JSONSchemaOrJSONReferenceVisitor,
            definitions: JSONSchemaDefinitionsVisitor,
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
            links: JSONSchemaLinksVisitor,
            media: {
              $ref: '#/visitors/document/objects/Media'
            },
            readOnly: {
              $ref: '#/visitors/value'
            }
          }
        },
        JSONReference: {
          $visitor: JSONReferenceVisitor,
          fixedFields: {
            $ref: JSONReference$RefVisitor
          }
        },
        Media: {
          $visitor: MediaVisitor,
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
          $visitor: LinkDescriptionVisitor,
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
            targetSchema: JSONSchemaOrJSONReferenceVisitor,
            mediaType: {
              $ref: '#/visitors/value'
            },
            method: {
              $ref: '#/visitors/value'
            },
            encType: {
              $ref: '#/visitors/value'
            },
            schema: JSONSchemaOrJSONReferenceVisitor
          }
        }
      }
    }
  }
};
export default specification;