import { ArrayElement, ObjectElement, isStringElement, isElement, isMemberElement, isArrayElement, includesClasses, cloneDeep, toValue, assignSourceMap } from '@swagger-api/apidom-core';

/**
 * OpenAPI 2.0 specification elements.
 */
import InfoElement from "../../elements/Info.mjs";
import ContactElement from "../../elements/Contact.mjs";
import LicenseElement from "../../elements/License.mjs";
import PathsElement from "../../elements/Paths.mjs";
import PathItemElement from "../../elements/PathItem.mjs";
import OperationElement from "../../elements/Operation.mjs";
import ExternalDocumentationElement from "../../elements/ExternalDocumentation.mjs";
import ParameterElement from "../../elements/Parameter.mjs";
import ItemsElement from "../../elements/Items.mjs";
import ResponsesElement from "../../elements/Responses.mjs";
import ResponseElement from "../../elements/Response.mjs";
import HeadersElement from "../../elements/Headers.mjs";
import ExampleElement from "../../elements/Example.mjs";
import HeaderElement from "../../elements/Header.mjs";
import TagElement from "../../elements/Tag.mjs";
import SchemaElement from "../../elements/Schema.mjs";
import XmlElement from "../../elements/Xml.mjs";
import DefinitionsElement from "../../elements/Definitions.mjs";
import ParametersDefinitionsElement from "../../elements/ParametersDefinitions.mjs";
import ResponsesDefinitionsElement from "../../elements/ResponsesDefinitions.mjs";
import SecurityDefinitionsElement from "../../elements/SecurityDefinitions.mjs";
import SecuritySchemeElement from "../../elements/SecurityScheme.mjs";
import ScopesElement from "../../elements/Scopes.mjs";
import SecurityRequirementElement from "../../elements/SecurityRequirement.mjs"; // non-concrete Elements (NCEs)
import OperationConsumesElement from "../../elements/nces/OperationConsumes.mjs";
import OperationParametersElement from "../../elements/nces/OperationParameters.mjs";
import OperationProducesElement from "../../elements/nces/OperationProduces.mjs";
import OperationSchemesElement from "../../elements/nces/OperationSchemes.mjs";
import OperationSecurityElement from "../../elements/nces/OperationSecurity.mjs";
import OperationTagsElement from "../../elements/nces/OperationTags.mjs";
import PathItemParametersElement from "../../elements/nces/PathItemParameters.mjs";
import SwaggerSchemesElement from "../../elements/nces/SwaggerSchemes.mjs";
import SwaggerConsumesElement from "../../elements/nces/SwaggerConsumes.mjs";
import SwaggerProducesElement from "../../elements/nces/SwaggerProduces.mjs";
import SwaggerSecurityElement from "../../elements/nces/SwaggerSecurity.mjs";
import SwaggerTagsElement from "../../elements/nces/SwaggerTags.mjs";
import { getNodeType } from "../../traversal/visitor.mjs";
/**
 * This plugin is specific to YAML 1.2 format, which allows defining key-value pairs
 * with empty key, empty value, or both. If the value is not provided in YAML format,
 * this plugin compensates for this missing value with the most appropriate semantic element type.
 *
 * https://yaml.org/spec/1.2.2/#72-empty-nodes
 *
 * @example
 *
 * ```yaml
 * swagger: "2.0"
 * info:
 * ```
 * Refracting result without this plugin:
 *
 *  (SwaggerElement
 *    (MemberElement
 *      (StringElement)
 *      (SwaggerVersionElement))
 *    (MemberElement
 *      (StringElement)
 *      (StringElement))
 *
 * Refracting result with this plugin:
 *
 *  (SwaggerElement
 *    (MemberElement
 *      (StringElement)
 *      (SwaggerVersionElement))
 *    (MemberElement
 *      (StringElement)
 *      (InfoElement))
 */
const isEmptyElement = element => isStringElement(element) && includesClasses(['yaml-e-node', 'yaml-e-scalar'], element);
const schema = {
  // concrete types handling (CTs)
  SwaggerElement: {
    info(...args) {
      return new InfoElement(...args);
    },
    schemes(...args) {
      return new SwaggerSchemesElement(...args);
    },
    consumes(...args) {
      return new SwaggerConsumesElement(...args);
    },
    produces(...args) {
      return new SwaggerProducesElement(...args);
    },
    paths(...args) {
      return new PathsElement(...args);
    },
    definitions(...args) {
      return new DefinitionsElement(...args);
    },
    parameters(...args) {
      return new ParametersDefinitionsElement(...args);
    },
    responses(...args) {
      return new ResponsesDefinitionsElement(...args);
    },
    securityDefinitions(...args) {
      return new SecurityDefinitionsElement(...args);
    },
    security(...args) {
      return new SwaggerSecurityElement(...args);
    },
    tags(...args) {
      return new SwaggerTagsElement(...args);
    },
    externalDocs(...args) {
      return new ExternalDocumentationElement(...args);
    }
  },
  InfoElement: {
    contact(...args) {
      return new ContactElement(...args);
    },
    license(...args) {
      return new LicenseElement(...args);
    }
  },
  PathsElement: {
    '[key: *]': function key(...args) {
      return new PathItemElement(...args);
    }
  },
  PathItemElement: {
    get(...args) {
      return new OperationElement(...args);
    },
    put(...args) {
      return new OperationElement(...args);
    },
    post(...args) {
      return new OperationElement(...args);
    },
    delete(...args) {
      return new OperationElement(...args);
    },
    options(...args) {
      return new OperationElement(...args);
    },
    head(...args) {
      return new OperationElement(...args);
    },
    patch(...args) {
      return new OperationElement(...args);
    },
    parameters(...args) {
      return new PathItemParametersElement(...args);
    }
  },
  OperationElement: {
    tags(...args) {
      return new OperationTagsElement(...args);
    },
    externalDocs(...args) {
      return new ExternalDocumentationElement(...args);
    },
    consumes(...args) {
      return new OperationConsumesElement(...args);
    },
    produces(...args) {
      return new OperationProducesElement(...args);
    },
    parameters(...args) {
      return new OperationParametersElement(...args);
    },
    responses(...args) {
      return new ResponsesElement(...args);
    },
    schemes(...args) {
      return new OperationSchemesElement(...args);
    },
    security(...args) {
      return new OperationSecurityElement(...args);
    }
  },
  ParameterElement: {
    schema(...args) {
      return new SchemaElement(...args);
    },
    items(...args) {
      return new ItemsElement(...args);
    }
  },
  ItemsElement: {
    items(...args) {
      return new ItemsElement(...args);
    }
  },
  ResponsesElement: {
    '[key: *]': function key(...args) {
      return new ResponseElement(...args);
    }
  },
  ResponseElement: {
    schema(...args) {
      return new SchemaElement(...args);
    },
    headers(...args) {
      return new HeadersElement(...args);
    },
    examples(...args) {
      return new ExampleElement(...args);
    }
  },
  HeadersElement: {
    '[key: *]': function key(...args) {
      return new HeaderElement(...args);
    }
  },
  HeaderElement: {
    items(...args) {
      return new ItemsElement(...args);
    }
  },
  TagElement: {
    externalDocs(...args) {
      return new ExternalDocumentationElement(...args);
    }
  },
  SchemaElement: {
    items(...args) {
      return new SchemaElement(...args);
    },
    allOf(...args) {
      const element = new ArrayElement(...args);
      element.classes.push('json-schema-allOf');
      return element;
    },
    properties(...args) {
      const element = new ObjectElement(...args);
      element.classes.push('json-schema-properties');
      return element;
    },
    additionalProperties(...args) {
      return new SchemaElement(...args);
    },
    xml(...args) {
      return new XmlElement(...args);
    },
    externalDocs(...args) {
      return new ExternalDocumentationElement(...args);
    }
  },
  DefinitionsElement: {
    '[key: *]': function key(...args) {
      return new SchemaElement(...args);
    }
  },
  ParametersDefinitionsElement: {
    '[key: *]': function key(...args) {
      return new ParameterElement(...args);
    }
  },
  ResponsesDefinitionsElement: {
    '[key: *]': function key(...args) {
      return new ResponseElement(...args);
    }
  },
  SecurityDefinitionsElement: {
    '[key: *]': function key(...args) {
      return new SecuritySchemeElement(...args);
    }
  },
  SecuritySchemeElement: {
    scopes(...args) {
      return new ScopesElement(...args);
    }
  },
  SecurityRequirementElement: {
    '[key: *]': function key(...args) {
      return new ArrayElement(...args);
    }
  },
  // non-concrete types handling (NCEs)
  [OperationParametersElement.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new ParameterElement(...args);
    }
  },
  [OperationSecurityElement.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new SecurityRequirementElement(...args);
    }
  },
  [PathItemParametersElement.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new ParameterElement(...args);
    }
  },
  [SwaggerSecurityElement.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new SecurityRequirementElement(...args);
    }
  },
  [SwaggerTagsElement.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new TagElement(...args);
    }
  },
  'json-schema-properties': {
    '[key: *]': function key(...args) {
      return new SchemaElement(...args);
    }
  },
  'json-schema-allOf': {
    '<*>': function asterisk(...args) {
      return new SchemaElement(...args);
    }
  }
};
const findElementFactory = (ancestor, keyName) => {
  const elementType = getNodeType(ancestor); // @ts-ignore
  const keyMapping = schema[elementType] || schema[toValue(ancestor.classes.first)];
  return typeof keyMapping === 'undefined' ? undefined : Object.prototype.hasOwnProperty.call(keyMapping, '[key: *]') ? keyMapping['[key: *]'] : keyMapping[keyName];
};

/**
 * @public
 */
const plugin = () => () => ({
  visitor: {
    StringElement(element, key, parent, path, ancestors) {
      if (!isEmptyElement(element)) return undefined;
      const lineage = [...ancestors, parent].filter(isElement);
      const parentElement = lineage[lineage.length - 1]; // @TODO(vladimir.gorej@gmail.com): can be replaced by Array.prototype.at in future
      let elementFactory;
      let context;
      if (isArrayElement(parentElement)) {
        context = element;
        elementFactory = findElementFactory(parentElement, '<*>');
      } else if (isMemberElement(parentElement)) {
        context = lineage[lineage.length - 2]; // @TODO(vladimir.gorej@gmail.com): can be replaced by Array.prototype.at in future
        elementFactory = findElementFactory(context, toValue(parentElement.key));
      }

      // no element factory found
      if (typeof elementFactory !== 'function') return undefined;
      const result = elementFactory.call({
        context
      }, undefined, cloneDeep(element.meta), cloneDeep(element.attributes));
      return assignSourceMap(result, element);
    }
  }
});
export default plugin;