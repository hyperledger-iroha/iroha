"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
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
var _Servers = _interopRequireDefault(require("../../elements/nces/Servers.cjs"));
var _Security = _interopRequireDefault(require("../../elements/nces/Security.cjs"));
var _Tags = _interopRequireDefault(require("../../elements/nces/Tags.cjs"));
var _ServerVariables = _interopRequireDefault(require("../../elements/nces/ServerVariables.cjs"));
var _ComponentsSchemas = _interopRequireDefault(require("../../elements/nces/ComponentsSchemas.cjs"));
var _ComponentsResponses = _interopRequireDefault(require("../../elements/nces/ComponentsResponses.cjs"));
var _ComponentsParameters = _interopRequireDefault(require("../../elements/nces/ComponentsParameters.cjs"));
var _ComponentsExamples = _interopRequireDefault(require("../../elements/nces/ComponentsExamples.cjs"));
var _ComponentsRequestBodies = _interopRequireDefault(require("../../elements/nces/ComponentsRequestBodies.cjs"));
var _ComponentsHeaders = _interopRequireDefault(require("../../elements/nces/ComponentsHeaders.cjs"));
var _ComponentsSecuritySchemes = _interopRequireDefault(require("../../elements/nces/ComponentsSecuritySchemes.cjs"));
var _ComponentsLinks = _interopRequireDefault(require("../../elements/nces/ComponentsLinks.cjs"));
var _ComponentsCallbacks = _interopRequireDefault(require("../../elements/nces/ComponentsCallbacks.cjs"));
var _PathItemServers = _interopRequireDefault(require("../../elements/nces/PathItemServers.cjs"));
var _PathItemParameters = _interopRequireDefault(require("../../elements/nces/PathItemParameters.cjs"));
var _OperationParameters = _interopRequireDefault(require("../../elements/nces/OperationParameters.cjs"));
var _ParameterExamples = _interopRequireDefault(require("../../elements/nces/ParameterExamples.cjs"));
var _ParameterContent = _interopRequireDefault(require("../../elements/nces/ParameterContent.cjs"));
var _OperationTags = _interopRequireDefault(require("../../elements/nces/OperationTags.cjs"));
var _OperationCallbacks = _interopRequireDefault(require("../../elements/nces/OperationCallbacks.cjs"));
var _OperationSecurity = _interopRequireDefault(require("../../elements/nces/OperationSecurity.cjs"));
var _OperationServers = _interopRequireDefault(require("../../elements/nces/OperationServers.cjs"));
var _RequestBodyContent = _interopRequireDefault(require("../../elements/nces/RequestBodyContent.cjs"));
var _MediaTypeExamples = _interopRequireDefault(require("../../elements/nces/MediaTypeExamples.cjs"));
var _MediaTypeEncoding = _interopRequireDefault(require("../../elements/nces/MediaTypeEncoding.cjs"));
var _EncodingHeaders = _interopRequireDefault(require("../../elements/nces/EncodingHeaders.cjs"));
var _ResponseHeaders = _interopRequireDefault(require("../../elements/nces/ResponseHeaders.cjs"));
var _ResponseContent = _interopRequireDefault(require("../../elements/nces/ResponseContent.cjs"));
var _ResponseLinks = _interopRequireDefault(require("../../elements/nces/ResponseLinks.cjs"));
var _DiscriminatorMapping = _interopRequireDefault(require("../../elements/nces/DiscriminatorMapping.cjs"));
var _OAuthFlowScopes = _interopRequireDefault(require("../../elements/nces/OAuthFlowScopes.cjs"));
var _LinkParameters = _interopRequireDefault(require("../../elements/nces/LinkParameters.cjs"));
var _HeaderExamples = _interopRequireDefault(require("../../elements/nces/HeaderExamples.cjs"));
var _HeaderContent = _interopRequireDefault(require("../../elements/nces/HeaderContent.cjs"));
var _visitor = require("../../traversal/visitor.cjs");
/**
 * OpenAPI 3.0.x specification elements.
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
 * openapi: 3.0.4
 * info:
 * ```
 * Refracting result without this plugin:
 *
 *  (OpenApi3_0Element
 *    (MemberElement
 *      (StringElement)
 *      (OpenapiElement))
 *    (MemberElement
 *      (StringElement)
 *      (StringElement))
 *
 * Refracting result with this plugin:
 *
 *  (OpenApi3_0Element
 *    (MemberElement
 *      (StringElement)
 *      (OpenapiElement))
 *    (MemberElement
 *      (StringElement)
 *      (InfoElement))
 */

const isEmptyElement = element => (0, _apidomCore.isStringElement)(element) && (0, _apidomCore.includesClasses)(['yaml-e-node', 'yaml-e-scalar'], element);
const schema = {
  // concrete types handling (CTs)
  OpenApi3_0Element: {
    info(...args) {
      return new _Info.default(...args);
    },
    servers(...args) {
      return new _Servers.default(...args);
    },
    paths(...args) {
      return new _Paths.default(...args);
    },
    components(...args) {
      return new _Components.default(...args);
    },
    security(...args) {
      return new _Security.default(...args);
    },
    tags(...args) {
      return new _Tags.default(...args);
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
      return new _ServerVariables.default(...args);
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
      return new _PathItemServers.default(...args);
    },
    parameters(...args) {
      return new _PathItemParameters.default(...args);
    }
  },
  OperationElement: {
    tags(...args) {
      return new _OperationTags.default(...args);
    },
    externalDocs(...args) {
      return new _ExternalDocumentation.default(...args);
    },
    parameters(...args) {
      return new _OperationParameters.default(...args);
    },
    requestBody(...args) {
      return new _RequestBody.default(...args);
    },
    responses(...args) {
      return new _Responses.default(...args);
    },
    callbacks(...args) {
      return new _OperationCallbacks.default(...args);
    },
    security(...args) {
      return new _OperationSecurity.default(...args);
    },
    servers(...args) {
      return new _OperationServers.default(...args);
    }
  },
  ParameterElement: {
    schema(...args) {
      return new _Schema.default(...args);
    },
    examples(...args) {
      return new _ParameterExamples.default(...args);
    },
    content(...args) {
      return new _ParameterContent.default(...args);
    }
  },
  RequestBodyElement: {
    content(...args) {
      return new _RequestBodyContent.default(...args);
    }
  },
  MediaTypeElement: {
    schema(...args) {
      return new _Schema.default(...args);
    },
    examples(...args) {
      return new _MediaTypeExamples.default(...args);
    },
    encoding(...args) {
      return new _MediaTypeEncoding.default(...args);
    }
  },
  EncodingElement: {
    headers(...args) {
      return new _EncodingHeaders.default(...args);
    }
  },
  ResponsesElement: {
    '[key: *]': function key(...args) {
      return new _Response.default(...args);
    }
  },
  ResponseElement: {
    headers(...args) {
      return new _ResponseHeaders.default(...args);
    },
    content(...args) {
      return new _ResponseContent.default(...args);
    },
    links(...args) {
      return new _ResponseLinks.default(...args);
    }
  },
  CallbackElement: {
    '[key: *]': function key(...args) {
      return new _PathItem.default(...args);
    }
  },
  LinkElement: {
    parameters(...args) {
      return new _LinkParameters.default(...args);
    },
    server(...args) {
      return new _Server.default(...args);
    }
  },
  HeaderElement: {
    schema(...args) {
      return new _Schema.default(...args);
    },
    examples(...args) {
      return new _HeaderExamples.default(...args);
    },
    content(...args) {
      return new _HeaderContent.default(...args);
    }
  },
  ComponentsElement: {
    schemas(...args) {
      return new _ComponentsSchemas.default(...args);
    },
    responses(...args) {
      return new _ComponentsResponses.default(...args);
    },
    parameters(...args) {
      return new _ComponentsParameters.default(...args);
    },
    examples(...args) {
      return new _ComponentsExamples.default(...args);
    },
    requestBodies(...args) {
      return new _ComponentsRequestBodies.default(...args);
    },
    headers(...args) {
      return new _ComponentsHeaders.default(...args);
    },
    securitySchemes(...args) {
      return new _ComponentsSecuritySchemes.default(...args);
    },
    links(...args) {
      return new _ComponentsLinks.default(...args);
    },
    callbacks(...args) {
      return new _ComponentsCallbacks.default(...args);
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
    definitions(...args) {
      const element = new _apidomCore.ObjectElement(...args);
      element.classes.push('json-schema-definitions');
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
    items(...args) {
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
      return new _DiscriminatorMapping.default(...args);
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
      return new _OAuthFlowScopes.default(...args);
    }
  },
  // non-concrete types handling (NCEs)
  [_ServerVariables.default.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _ServerVariable.default(...args);
    }
  },
  [_ComponentsSchemas.default.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Schema.default(...args);
    }
  },
  [_ComponentsResponses.default.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Response.default(...args);
    }
  },
  [_ComponentsParameters.default.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Parameter.default(...args);
    }
  },
  [_ComponentsExamples.default.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Example.default(...args);
    }
  },
  [_ComponentsRequestBodies.default.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _RequestBody.default(...args);
    }
  },
  [_ComponentsHeaders.default.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Header.default(...args);
    }
  },
  [_ComponentsSecuritySchemes.default.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _SecurityScheme.default(...args);
    }
  },
  [_ComponentsLinks.default.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Link.default(...args);
    }
  },
  [_ComponentsCallbacks.default.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Callback.default(...args);
    }
  },
  [_OperationCallbacks.default.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Callback.default(...args);
    }
  },
  [_ParameterExamples.default.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Example.default(...args);
    }
  },
  [_ParameterContent.default.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _MediaType.default(...args);
    }
  },
  [_RequestBodyContent.default.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _MediaType.default(...args);
    }
  },
  [_MediaTypeExamples.default.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Example.default(...args);
    }
  },
  [_MediaTypeEncoding.default.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Encoding.default(...args);
    }
  },
  [_EncodingHeaders.default.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Header.default(...args);
    }
  },
  [_ResponseHeaders.default.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _Header.default(...args);
    }
  },
  [_ResponseContent.default.primaryClass]: {
    '[key: *]': function key(...args) {
      return new _MediaType.default(...args);
    }
  },
  [_ResponseLinks.default.primaryClass]: {
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
  [_Servers.default.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new _Server.default(...args);
    }
  },
  [_Security.default.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new _SecurityRequirement.default(...args);
    }
  },
  [_Tags.default.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new _Tag.default(...args);
    }
  },
  [_PathItemServers.default.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new _Server.default(...args);
    }
  },
  [_PathItemParameters.default.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new _Parameter.default(...args);
    }
  },
  [_OperationParameters.default.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new _Parameter.default(...args);
    }
  },
  [_OperationSecurity.default.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new _SecurityRequirement.default(...args);
    }
  },
  [_OperationServers.default.primaryClass]: {
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
const plugin = () => () => ({
  visitor: {
    StringElement(element, key, parent, path, ancestors) {
      if (!isEmptyElement(element)) return undefined;
      const lineage = [...ancestors, parent].filter(_apidomCore.isElement);
      const parentElement = lineage[lineage.length - 1]; // @TODO(vladimir.gorej@gmail.com): can be replaced by Array.prototype.at in future
      let elementFactory;
      let context;
      if ((0, _apidomCore.isArrayElement)(parentElement)) {
        context = element;
        elementFactory = findElementFactory(parentElement, '<*>');
      } else if ((0, _apidomCore.isMemberElement)(parentElement)) {
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
});
var _default = exports.default = plugin;