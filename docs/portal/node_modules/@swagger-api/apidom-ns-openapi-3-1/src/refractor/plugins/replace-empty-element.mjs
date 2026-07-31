import { ArrayElement, ObjectElement, assignSourceMap, cloneDeep, toValue } from '@swagger-api/apidom-core';
import { ServersElement, SecurityElement, TagsElement, ServerVariablesElement, ComponentsSchemasElement, ComponentsResponsesElement, ComponentsParametersElement, ComponentsExamplesElement, ComponentsRequestBodiesElement, ComponentsHeadersElement, ComponentsSecuritySchemesElement, ComponentsLinksElement, ComponentsCallbacksElement, PathItemServersElement, PathItemParametersElement, OperationParametersElement, ParameterExamplesElement, ParameterContentElement, HeaderExamplesElement, HeaderContentElement, OperationTagsElement, OperationCallbacksElement, OperationSecurityElement, OperationServersElement, RequestBodyContentElement, MediaTypeExamplesElement, MediaTypeEncodingElement, EncodingHeadersElement, ResponseHeadersElement, ResponseContentElement, ResponseLinksElement, DiscriminatorMappingElement, OAuthFlowScopesElement } from '@swagger-api/apidom-ns-openapi-3-0';

/**
 * OpenAPI 3.1.0 specification elements.
 */
import InfoElement from "../../elements/Info.mjs";
import ContactElement from "../../elements/Contact.mjs";
import LicenseElement from "../../elements/License.mjs";
import PathsElement from "../../elements/Paths.mjs";
import PathItemElement from "../../elements/PathItem.mjs";
import ComponentsElement from "../../elements/Components.mjs";
import ExternalDocumentationElement from "../../elements/ExternalDocumentation.mjs";
import OperationElement from "../../elements/Operation.mjs";
import SchemaElement from "../../elements/Schema.mjs";
import RequestBodyElement from "../../elements/RequestBody.mjs";
import ResponsesElement from "../../elements/Responses.mjs";
import ResponseElement from "../../elements/Response.mjs";
import ServerElement from "../../elements/Server.mjs";
import DiscriminatorElement from "../../elements/Discriminator.mjs";
import XmlElement from "../../elements/Xml.mjs";
import OAuthFlowsElement from "../../elements/OAuthFlows.mjs";
import OAuthFlowElement from "../../elements/OAuthFlow.mjs";
import ServerVariableElement from "../../elements/ServerVariable.mjs";
import ParameterElement from "../../elements/Parameter.mjs";
import ExampleElement from "../../elements/Example.mjs";
import HeaderElement from "../../elements/Header.mjs";
import SecuritySchemeElement from "../../elements/SecurityScheme.mjs";
import LinkElement from "../../elements/Link.mjs";
import CallbackElement from "../../elements/Callback.mjs";
import MediaTypeElement from "../../elements/MediaType.mjs";
import EncodingElement from "../../elements/Encoding.mjs";
import SecurityRequirementElement from "../../elements/SecurityRequirement.mjs";
import TagElement from "../../elements/Tag.mjs"; // non-concrete Elements (NCEs)
import ComponentsPathItemsElement from "../../elements/nces/ComponentsPathItems.mjs";
import WebhooksElement from "../../elements/nces/Webhooks.mjs";
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
 * openapi: 3.1.0
 * info:
 * ```
 * Refracting result without this plugin:
 *
 *  (OpenApi3_1Element
 *    (MemberElement
 *      (StringElement)
 *      (OpenapiElement))
 *    (MemberElement
 *      (StringElement)
 *      (StringElement))
 *
 * Refracting result with this plugin:
 *
 *  (OpenApi3_1Element
 *    (MemberElement
 *      (StringElement)
 *      (OpenapiElement))
 *    (MemberElement
 *      (StringElement)
 *      (InfoElement))
 */

const schema = {
  // concrete types handling (CTs)
  OpenApi3_1Element: {
    info(...args) {
      return new InfoElement(...args);
    },
    servers(...args) {
      return new ServersElement(...args);
    },
    paths(...args) {
      return new PathsElement(...args);
    },
    webhooks(...args) {
      return new WebhooksElement(...args);
    },
    components(...args) {
      return new ComponentsElement(...args);
    },
    security(...args) {
      return new SecurityElement(...args);
    },
    tags(...args) {
      return new TagsElement(...args);
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
  ServerElement: {
    variables(...args) {
      return new ServerVariablesElement(...args);
    }
  },
  ServerVariableElement: {
    enum(...args) {
      return new ArrayElement(...args);
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
    trace(...args) {
      return new OperationElement(...args);
    },
    servers(...args) {
      return new PathItemServersElement(...args);
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
    parameters(...args) {
      return new OperationParametersElement(...args);
    },
    requestBody(...args) {
      return new RequestBodyElement(...args);
    },
    responses(...args) {
      return new ResponsesElement(...args);
    },
    callbacks(...args) {
      return new OperationCallbacksElement(...args);
    },
    security(...args) {
      return new OperationSecurityElement(...args);
    },
    servers(...args) {
      return new OperationServersElement(...args);
    }
  },
  ParameterElement: {
    schema(...args) {
      return new SchemaElement(...args);
    },
    examples(...args) {
      return new ParameterExamplesElement(...args);
    },
    content(...args) {
      return new ParameterContentElement(...args);
    }
  },
  RequestBodyElement: {
    content(...args) {
      return new RequestBodyContentElement(...args);
    }
  },
  MediaTypeElement: {
    schema(...args) {
      return new SchemaElement(...args);
    },
    examples(...args) {
      return new MediaTypeExamplesElement(...args);
    },
    encoding(...args) {
      return new MediaTypeEncodingElement(...args);
    }
  },
  EncodingElement: {
    headers(...args) {
      return new EncodingHeadersElement(...args);
    }
  },
  ResponsesElement: {
    '[key: *]': function key(...args) {
      return new ResponseElement(...args);
    }
  },
  ResponseElement: {
    headers(...args) {
      return new ResponseHeadersElement(...args);
    },
    content(...args) {
      return new ResponseContentElement(...args);
    },
    links(...args) {
      return new ResponseLinksElement(...args);
    }
  },
  CallbackElement: {
    '[key: *]': function key(...args) {
      return new PathItemElement(...args);
    }
  },
  LinkElement: {
    server(...args) {
      return new ServerElement(...args);
    }
  },
  HeaderElement: {
    schema(...args) {
      return new SchemaElement(...args);
    },
    examples(...args) {
      return new HeaderExamplesElement(...args);
    },
    content(...args) {
      return new HeaderContentElement(...args);
    }
  },
  ComponentsElement: {
    schemas(...args) {
      return new ComponentsSchemasElement(...args);
    },
    responses(...args) {
      return new ComponentsResponsesElement(...args);
    },
    parameters(...args) {
      return new ComponentsParametersElement(...args);
    },
    examples(...args) {
      return new ComponentsExamplesElement(...args);
    },
    requestBodies(...args) {
      return new ComponentsRequestBodiesElement(...args);
    },
    headers(...args) {
      return new ComponentsHeadersElement(...args);
    },
    securitySchemes(...args) {
      return new ComponentsSecuritySchemesElement(...args);
    },
    links(...args) {
      return new ComponentsLinksElement(...args);
    },
    callbacks(...args) {
      return new ComponentsCallbacksElement(...args);
    },
    pathItems(...args) {
      return new ComponentsPathItemsElement(...args);
    }
  },
  SecurityRequirementElement: {
    '[key: *]': function key(...args) {
      return new ArrayElement(...args);
    }
  },
  TagElement: {
    externalDocs(...args) {
      return new ExternalDocumentationElement(...args);
    }
  },
  SchemaElement: {
    $vocabulary(...args) {
      const element = new ObjectElement(...args);
      element.classes.push('json-schema-$vocabulary');
      return element;
    },
    $defs(...args) {
      const element = new ObjectElement(...args);
      element.classes.push('json-schema-$defs');
      return element;
    },
    allOf(...args) {
      const element = new ArrayElement(...args);
      element.classes.push('json-schema-allOf');
      return element;
    },
    anyOf(...args) {
      const element = new ArrayElement(...args);
      element.classes.push('json-schema-anyOf');
      return element;
    },
    oneOf(...args) {
      const element = new ArrayElement(...args);
      element.classes.push('json-schema-oneOf');
      return element;
    },
    not(...args) {
      return new SchemaElement(...args);
    },
    if(...args) {
      return new SchemaElement(...args);
    },
    then(...args) {
      return new SchemaElement(...args);
    },
    else(...args) {
      return new SchemaElement(...args);
    },
    dependentSchemas(...args) {
      const element = new ObjectElement(...args);
      element.classes.push('json-schema-dependentSchemas');
      return element;
    },
    prefixItems(...args) {
      const element = new ArrayElement(...args);
      element.classes.push('json-schema-prefixItems');
      return element;
    },
    items(...args) {
      return new SchemaElement(...args);
    },
    contains(...args) {
      return new SchemaElement(...args);
    },
    properties(...args) {
      const element = new ObjectElement(...args);
      element.classes.push('json-schema-properties');
      return element;
    },
    patternProperties(...args) {
      const element = new ObjectElement(...args);
      element.classes.push('json-schema-patternProperties');
      return element;
    },
    additionalProperties(...args) {
      return new SchemaElement(...args);
    },
    propertyNames(...args) {
      return new SchemaElement(...args);
    },
    unevaluatedItems(...args) {
      return new SchemaElement(...args);
    },
    unevaluatedProperties(...args) {
      return new SchemaElement(...args);
    },
    type(...args) {
      const element = new ArrayElement(...args);
      element.classes.push('json-schema-type');
      return element;
    },
    enum(...args) {
      const element = new ArrayElement(...args);
      element.classes.push('json-schema-enum');
      return element;
    },
    required(...args) {
      const element = new ArrayElement(...args);
      element.classes.push('json-schema-required');
      return element;
    },
    dependentRequired(...args) {
      const element = new ObjectElement(...args);
      element.classes.push('json-schema-dependentRequired');
      return element;
    },
    examples(...args) {
      const element = new ArrayElement(...args);
      element.classes.push('json-schema-examples');
      return element;
    },
    contentSchema(...args) {
      return new SchemaElement(...args);
    },
    discriminator(...args) {
      return new DiscriminatorElement(...args);
    },
    xml(...args) {
      return new XmlElement(...args);
    },
    externalDocs(...args) {
      return new ExternalDocumentationElement(...args);
    }
  },
  DiscriminatorElement: {
    mapping(...args) {
      return new DiscriminatorMappingElement(...args);
    }
  },
  SecuritySchemeElement: {
    flows(...args) {
      return new OAuthFlowsElement(...args);
    }
  },
  OAuthFlowsElement: {
    implicit(...args) {
      return new OAuthFlowElement(...args);
    },
    password(...args) {
      return new OAuthFlowElement(...args);
    },
    clientCredentials(...args) {
      return new OAuthFlowElement(...args);
    },
    authorizationCode(...args) {
      return new OAuthFlowElement(...args);
    }
  },
  OAuthFlowElement: {
    scopes(...args) {
      return new OAuthFlowScopesElement(...args);
    }
  },
  // non-concrete types handling (NCEs)
  [WebhooksElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new PathItemElement(...args);
    }
  },
  [ServerVariablesElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new ServerVariableElement(...args);
    }
  },
  [ComponentsSchemasElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new SchemaElement(...args);
    }
  },
  [ComponentsResponsesElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new ResponseElement(...args);
    }
  },
  [ComponentsParametersElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new ParameterElement(...args);
    }
  },
  [ComponentsExamplesElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new ExampleElement(...args);
    }
  },
  [ComponentsRequestBodiesElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new RequestBodyElement(...args);
    }
  },
  [ComponentsHeadersElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new HeaderElement(...args);
    }
  },
  [ComponentsSecuritySchemesElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new SecuritySchemeElement(...args);
    }
  },
  [ComponentsLinksElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new LinkElement(...args);
    }
  },
  [ComponentsCallbacksElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new CallbackElement(...args);
    }
  },
  [ComponentsPathItemsElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new PathItemElement(...args);
    }
  },
  [OperationCallbacksElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new CallbackElement(...args);
    }
  },
  [ParameterExamplesElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new ExampleElement(...args);
    }
  },
  [ParameterContentElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new MediaTypeElement(...args);
    }
  },
  [RequestBodyContentElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new MediaTypeElement(...args);
    }
  },
  [MediaTypeExamplesElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new ExampleElement(...args);
    }
  },
  [MediaTypeEncodingElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new EncodingElement(...args);
    }
  },
  [EncodingHeadersElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new HeaderElement(...args);
    }
  },
  [ResponseHeadersElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new HeaderElement(...args);
    }
  },
  [ResponseContentElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new MediaTypeElement(...args);
    }
  },
  [ResponseLinksElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new LinkElement(...args);
    }
  },
  'json-schema-$defs': {
    '[key: *]': function key(...args) {
      return new SchemaElement(...args);
    }
  },
  'json-schema-dependentSchemas': {
    '[key: *]': function key(...args) {
      return new SchemaElement(...args);
    }
  },
  'json-schema-properties': {
    '[key: *]': function key(...args) {
      return new SchemaElement(...args);
    }
  },
  [ServersElement.primaryClass]: {
    '<*>': (...args) => new ServerElement(...args)
  },
  [SecurityElement.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new SecurityRequirementElement(...args);
    }
  },
  [TagsElement.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new TagElement(...args);
    }
  },
  [PathItemServersElement.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new ServerElement(...args);
    }
  },
  [PathItemParametersElement.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new ParameterElement(...args);
    }
  },
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
  [OperationServersElement.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new ServerElement(...args);
    }
  },
  'json-schema-allOf': {
    '<*>': function asterisk(...args) {
      return new SchemaElement(...args);
    }
  },
  'json-schema-anyOf': {
    '<*>': function asterisk(...args) {
      return new SchemaElement(...args);
    }
  },
  'json-schema-oneOf': {
    '<*>': function asterisk(...args) {
      return new SchemaElement(...args);
    }
  },
  'json-schema-prefixItems': {
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
const plugin = () => ({
  predicates
}) => {
  const isEmptyElement = element => predicates.isStringElement(element) && predicates.includesClasses(['yaml-e-node', 'yaml-e-scalar'], element);
  return {
    visitor: {
      StringElement(element, key, parent, path, ancestors) {
        if (!isEmptyElement(element)) return undefined;
        const lineage = [...ancestors, parent].filter(predicates.isElement);
        const parentElement = lineage[lineage.length - 1]; // @TODO(vladimir.gorej@gmail.com): can be replaced by Array.prototype.at in future
        let elementFactory;
        let context;
        if (predicates.isArrayElement(parentElement)) {
          context = element;
          elementFactory = findElementFactory(parentElement, '<*>');
        } else if (predicates.isMemberElement(parentElement)) {
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
  };
};
export default plugin;