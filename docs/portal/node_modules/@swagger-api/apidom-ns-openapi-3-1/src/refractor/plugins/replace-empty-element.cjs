"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _Info = _interopRequireDefault(require("../../elements/Info.cjs"));
var _Contact = _interopRequireDefault(require("../../elements/Contact.cjs"));
var _License = _interopRequireDefault(require("../../elements/License.cjs"));
var _Paths = _interopRequireDefault(require("../../elements/Paths.cjs"));
var _PathItem = _interopRequireDefault(require("../../elements/PathItem.cjs"));
var _Components = _interopRequireDefault(require("../../elements/Components.cjs"));
var _ExternalDocumentation = _interopRequireDefault(require("../../elements/ExternalDocumentation.cjs"));
var _Operation = _interopRequireDefault(require("../../elements/Operation.cjs"));
var _Schema = _interopRequireDefault(require("../../elements/Schema.cjs"));
var _RequestBody = _interopRequireDefault(require("../../elements/RequestBody.cjs"));
var _Responses = _interopRequireDefault(require("../../elements/Responses.cjs"));
var _Response = _interopRequireDefault(require("../../elements/Response.cjs"));
var _Server = _interopRequireDefault(require("../../elements/Server.cjs"));
var _Discriminator = _interopRequireDefault(require("../../elements/Discriminator.cjs"));
var _Xml = _interopRequireDefault(require("../../elements/Xml.cjs"));
var _OAuthFlows = _interopRequireDefault(require("../../elements/OAuthFlows.cjs"));
var _OAuthFlow = _interopRequireDefault(require("../../elements/OAuthFlow.cjs"));
var _ServerVariable = _interopRequireDefault(require("../../elements/ServerVariable.cjs"));
var _Parameter = _interopRequireDefault(require("../../elements/Parameter.cjs"));
var _Example = _interopRequireDefault(require("../../elements/Example.cjs"));
var _Header = _interopRequireDefault(require("../../elements/Header.cjs"));
var _SecurityScheme = _interopRequireDefault(require("../../elements/SecurityScheme.cjs"));
var _Link = _interopRequireDefault(require("../../elements/Link.cjs"));
var _Callback = _interopRequireDefault(require("../../elements/Callback.cjs"));
var _MediaType = _interopRequireDefault(require("../../elements/MediaType.cjs"));
var _Encoding = _interopRequireDefault(require("../../elements/Encoding.cjs"));
var _SecurityRequirement = _interopRequireDefault(require("../../elements/SecurityRequirement.cjs"));
var _Tag = _interopRequireDefault(require("../../elements/Tag.cjs"));
var _ComponentsPathItems = _interopRequireDefault(require("../../elements/nces/ComponentsPathItems.cjs"));
var _Webhooks = _interopRequireDefault(require("../../elements/nces/Webhooks.cjs"));
var _visitor = require("../../traversal/visitor.cjs");
/**
 * OpenAPI 3.1.0 specification elements.
 */

// non-concrete Elements (NCEs)

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
      return new _Info.default(...args);
    },
    servers(...args) {
      return new _apidomNsOpenapi.ServersElement(...args);
    },
    paths(...args) {
      return new _Paths.default(...args);
    },
    webhooks(...args) {
      return new _Webhooks.default(...args);
    },
    components(...args) {
      return new _Components.default(...args);
    },
    security(...args) {
      return new _apidomNsOpenapi.SecurityElement(...args);
    },
    tags(...args) {
      return new _apidomNsOpenapi.TagsElement(...args);
    },
    externalDocs(...args) {
      return new _ExternalDocumentation.default(...args);
    }
  },
  InfoElement: {
    contact(...args) {
      return new _Contact.default(...args);
    },
    license(...args) {
      return new _License.default(...args);
    }
  },
  ServerElement: {
    variables(...args) {
      return new _apidomNsOpenapi.ServerVariablesElement(...args);
    }
  },
  ServerVariableElement: {
    enum(...args) {
      return new _apidomCore.ArrayElement(...args);
    }
  },
  PathsElement: {
    '[key: *]': function key(...args) {
      return new _PathItem.default(...args);
    }
  },
  PathItemElement: {
    get(...args) {
      return new _Operation.default(...args);
    },
    put(...args) {
      return new _Operation.default(...args);
    },
    post(...args) {
      return new _Operation.default(...args);
    },
    delete(...args) {
      return new _Operation.default(...args);
    },
    options(...args) {
      return new _Operation.default(...args);
    },
    head(...args) {
      return new _Operation.default(...args);
    },
    patch(...args) {
      return new _Operation.default(...args);
    },
    trace(...args) {
      return new _Operation.default(...args);
    },
    servers(...args) {
      return new _apidomNsOpenapi.PathItemServersElement(...args);
    },
    parameters(...args) {
      return new _apidomNsOpenapi.PathItemParametersElement(...args);
    }
  },
  OperationElement: {
    tags(...args) {
      return new _apidomNsOpenapi.OperationTagsElement(...args);
    },
    externalDocs(...args) {
      return new _ExternalDocumentation.default(...args);
    },
    parameters(...args) {
      return new _apidomNsOpenapi.OperationParametersElement(...args);
    },
    requestBody(...args) {
      return new _RequestBody.default(...args);
    },
    responses(...args) {
      return new _Responses.default(...args);
    },
    callbacks(...args) {
      return new _apidomNsOpenapi.OperationCallbacksElement(...args);
    },
    security(...args) {
      return new _apidomNsOpenapi.OperationSecurityElement(...args);
    },
    servers(...args) {
      return new _apidomNsOpenapi.OperationServersElement(...args);
    }
  },
  ParameterElement: {
    schema(...args) {
      return new _Schema.default(...args);
    },
    examples(...args) {
      return new _apidomNsOpenapi.ParameterExamplesElement(...args);
    },
    content(...args) {
      return new _apidomNsOpenapi.ParameterContentElement(...args);
    }
  },
  RequestBodyElement: {
    content(...args) {
      return new _apidomNsOpenapi.RequestBodyContentElement(...args);
    }
  },
  MediaTypeElement: {
    schema(...args) {
      return new _Schema.default(...args);
    },
    examples(...args) {
      return new _apidomNsOpenapi.MediaTypeExamplesElement(...args);
    },
    encoding(...args) {
      return new _apidomNsOpenapi.MediaTypeEncodingElement(...args);
    }
  },
  EncodingElement: {
    headers(...args) {
      return new _apidomNsOpenapi.EncodingHeadersElement(...args);
    }
  },
  ResponsesElement: {
    '[key: *]': function key(...args) {
      return new _Response.default(...args);
    }
  },
  ResponseElement: {
    headers(...args) {
      return new _apidomNsOpenapi.ResponseHeadersElement(...args);
    },
    content(...args) {
      return new _apidomNsOpenapi.ResponseContentElement(...args);
    },
    links(...args) {
      return new _apidomNsOpenapi.ResponseLinksElement(...args);
    }
  },
  CallbackElement: {
    '[key: *]': function key(...args) {
      return new _PathItem.default(...args);
    }
  },
  LinkElement: {
    server(...args) {
      return new _Server.default(...args);
    }
  },
  HeaderElement: {
    schema(...args) {
      return new _Schema.default(...args);
    },
    examples(...args) {
      return new _apidomNsOpenapi.HeaderExamplesElement(...args);
    },
    content(...args) {
      return new _apidomNsOpenapi.HeaderContentElement(...args);
    }
  },
  ComponentsElement: {
    schemas(...args) {
      return new _apidomNsOpenapi.ComponentsSchemasElement(...args);
    },
    responses(...args) {
      return new _apidomNsOpenapi.ComponentsResponsesElement(...args);
    },
    parameters(...args) {
      return new _apidomNsOpenapi.ComponentsParametersElement(...args);
    },
    examples(...args) {
      return new _apidomNsOpenapi.ComponentsExamplesElement(...args);
    },
    requestBodies(...args) {
      return new _apidomNsOpenapi.ComponentsRequestBodiesElement(...args);
    },
    headers(...args) {
      return new _apidomNsOpenapi.ComponentsHeadersElement(...args);
    },
    securitySchemes(...args) {
      return new _apidomNsOpenapi.ComponentsSecuritySchemesElement(...args);
    },
    links(...args) {
      return new _apidomNsOpenapi.ComponentsLinksElement(...args);
    },
    callbacks(...args) {
      return new _apidomNsOpenapi.ComponentsCallbacksElement(...args);
    },
    pathItems(...args) {
      return new _ComponentsPathItems.default(...args);
    }
  },
  SecurityRequirementElement: {
    '[key: *]': function key(...args) {
      return new _apidomCore.ArrayElement(...args);
    }
  },
  TagElement: {
    externalDocs(...args) {
      return new _ExternalDocumentation.default(...args);
    }
  },
  SchemaElement: {
    $vocabulary(...args) {
      const element = new _apidomCore.ObjectElement(...args);
      element.classes.push('json-schema-$vocabulary');
      return element;
    },
    $defs(...args) {
      const element = new _apidomCore.ObjectElement(...args);
      element.classes.push('json-schema-$defs');
      return element;
    },
    allOf(...args) {
      const element = new _apidomCore.ArrayElement(...args);
      element.classes.push('json-schema-allOf');
      return element;
    },
    anyOf(...args) {
      const element = new _apidomCore.ArrayElement(...args);
      element.classes.push('json-schema-anyOf');
      return element;
    },
    oneOf(...args) {
      const element = new _apidomCore.ArrayElement(...args);
      element.classes.push('json-schema-oneOf');
      return element;
    },
    not(...args) {
      return new _Schema.default(...args);
    },
    if(...args) {
      return new _Schema.default(...args);
    },
    then(...args) {
      return new _Schema.default(...args);
    },
    else(...args) {
      return new _Schema.default(...args);
    },
    dependentSchemas(...args) {
      const element = new _apidomCore.ObjectElement(...args);
      element.classes.push('json-schema-dependentSchemas');
      return element;
    },
    prefixItems(...args) {
      const element = new _apidomCore.ArrayElement(...args);
      element.classes.push('json-schema-prefixItems');
      return element;
    },
    items(...args) {
      return new _Schema.default(...args);
    },
    contains(...args) {
      return new _Schema.default(...args);
    },
    properties(...args) {
      const element = new _apidomCore.ObjectElement(...args);
      element.classes.push('json-schema-properties');
      return element;
    },
    patternProperties(...args) {
      const element = new _apidomCore.ObjectElement(...args);
      element.classes.push('json-schema-patternProperties');
      return element;
    },
    additionalProperties(...args) {
      return new _Schema.default(...args);
    },
    propertyNames(...args) {
      return new _Schema.default(...args);
    },
    unevaluatedItems(...args) {
      return new _Schema.default(...args);
    },
    unevaluatedProperties(...args) {
      return new _Schema.default(...args);
    },
    type(...args) {
      const element = new _apidomCore.ArrayElement(...args);
      element.classes.push('json-schema-type');
      return element;
    },
    enum(...args) {
      const element = new _apidomCore.ArrayElement(...args);
      element.classes.push('json-schema-enum');
      return element;
    },
    required(...args) {
      const element = new _apidomCore.ArrayElement(...args);
      element.classes.push('json-schema-required');
      return element;
    },
    dependentRequired(...args) {
      const element = new _apidomCore.ObjectElement(...args);
      element.classes.push('json-schema-dependentRequired');
      return element;
    },
    examples(...args) {
      const element = new _apidomCore.ArrayElement(...args);
      element.classes.push('json-schema-examples');
      return element;
    },
    contentSchema(...args) {
      return new _Schema.default(...args);
    },
    discriminator(...args) {
      return new _Discriminator.default(...args);
    },
    xml(...args) {
      return new _Xml.default(...args);
    },
    externalDocs(...args) {
      return new _ExternalDocumentation.default(...args);
    }
  },
  DiscriminatorElement: {
    mapping(...args) {
      return new _apidomNsOpenapi.DiscriminatorMappingElement(...args);
    }
  },
  SecuritySchemeElement: {
    flows(...args) {
      return new _OAuthFlows.default(...args);
    }
  },
  OAuthFlowsElement: {
    implicit(...args) {
      return new _OAuthFlow.default(...args);
    },
    password(...args) {
      return new _OAuthFlow.default(...args);
    },
    clientCredentials(...args) {
      return new _OAuthFlow.default(...args);
    },
    authorizationCode(...args) {
      return new _OAuthFlow.default(...args);
    }
  },
  OAuthFlowElement: {
    scopes(...args) {
      return new _apidomNsOpenapi.OAuthFlowScopesElement(...args);
    }
  },
  // non-concrete types handling (NCEs)
  [_Webhooks.default.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _PathItem.default(...args);
    }
  },
  [_apidomNsOpenapi.ServerVariablesElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _ServerVariable.default(...args);
    }
  },
  [_apidomNsOpenapi.ComponentsSchemasElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Schema.default(...args);
    }
  },
  [_apidomNsOpenapi.ComponentsResponsesElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Response.default(...args);
    }
  },
  [_apidomNsOpenapi.ComponentsParametersElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Parameter.default(...args);
    }
  },
  [_apidomNsOpenapi.ComponentsExamplesElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Example.default(...args);
    }
  },
  [_apidomNsOpenapi.ComponentsRequestBodiesElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _RequestBody.default(...args);
    }
  },
  [_apidomNsOpenapi.ComponentsHeadersElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Header.default(...args);
    }
  },
  [_apidomNsOpenapi.ComponentsSecuritySchemesElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _SecurityScheme.default(...args);
    }
  },
  [_apidomNsOpenapi.ComponentsLinksElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Link.default(...args);
    }
  },
  [_apidomNsOpenapi.ComponentsCallbacksElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Callback.default(...args);
    }
  },
  [_ComponentsPathItems.default.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _PathItem.default(...args);
    }
  },
  [_apidomNsOpenapi.OperationCallbacksElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Callback.default(...args);
    }
  },
  [_apidomNsOpenapi.ParameterExamplesElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Example.default(...args);
    }
  },
  [_apidomNsOpenapi.ParameterContentElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _MediaType.default(...args);
    }
  },
  [_apidomNsOpenapi.RequestBodyContentElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _MediaType.default(...args);
    }
  },
  [_apidomNsOpenapi.MediaTypeExamplesElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Example.default(...args);
    }
  },
  [_apidomNsOpenapi.MediaTypeEncodingElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Encoding.default(...args);
    }
  },
  [_apidomNsOpenapi.EncodingHeadersElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Header.default(...args);
    }
  },
  [_apidomNsOpenapi.ResponseHeadersElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Header.default(...args);
    }
  },
  [_apidomNsOpenapi.ResponseContentElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _MediaType.default(...args);
    }
  },
  [_apidomNsOpenapi.ResponseLinksElement.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Link.default(...args);
    }
  },
  'json-schema-$defs': {
    '[key: *]': function key(...args) {
      return new _Schema.default(...args);
    }
  },
  'json-schema-dependentSchemas': {
    '[key: *]': function key(...args) {
      return new _Schema.default(...args);
    }
  },
  'json-schema-properties': {
    '[key: *]': function key(...args) {
      return new _Schema.default(...args);
    }
  },
  [_apidomNsOpenapi.ServersElement.primaryClass]: {
    '<*>': (...args) => new _Server.default(...args)
  },
  [_apidomNsOpenapi.SecurityElement.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new _SecurityRequirement.default(...args);
    }
  },
  [_apidomNsOpenapi.TagsElement.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new _Tag.default(...args);
    }
  },
  [_apidomNsOpenapi.PathItemServersElement.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new _Server.default(...args);
    }
  },
  [_apidomNsOpenapi.PathItemParametersElement.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new _Parameter.default(...args);
    }
  },
  [_apidomNsOpenapi.OperationParametersElement.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new _Parameter.default(...args);
    }
  },
  [_apidomNsOpenapi.OperationSecurityElement.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new _SecurityRequirement.default(...args);
    }
  },
  [_apidomNsOpenapi.OperationServersElement.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new _Server.default(...args);
    }
  },
  'json-schema-allOf': {
    '<*>': function asterisk(...args) {
      return new _Schema.default(...args);
    }
  },
  'json-schema-anyOf': {
    '<*>': function asterisk(...args) {
      return new _Schema.default(...args);
    }
  },
  'json-schema-oneOf': {
    '<*>': function asterisk(...args) {
      return new _Schema.default(...args);
    }
  },
  'json-schema-prefixItems': {
    '<*>': function asterisk(...args) {
      return new _Schema.default(...args);
    }
  }
};
const findElementFactory = (ancestor, keyName) => {
  const elementType = (0, _visitor.getNodeType)(ancestor); // @ts-ignore
  const keyMapping = schema[elementType] || schema[(0, _apidomCore.toValue)(ancestor.classes.first)];
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
          elementFactory = findElementFactory(context, (0, _apidomCore.toValue)(parentElement.key));
        }

        // no element factory found
        if (typeof elementFactory !== 'function') return undefined;
        const result = elementFactory.call({
          context
        }, undefined, (0, _apidomCore.cloneDeep)(element.meta), (0, _apidomCore.cloneDeep)(element.attributes));
        return (0, _apidomCore.assignSourceMap)(result, element);
      }
    }
  };
};
var _default = exports.default = plugin;