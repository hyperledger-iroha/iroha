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
var _Operation = _interopRequireDefault(require("../../elements/Operation.cjs"));
var _ExternalDocumentation = _interopRequireDefault(require("../../elements/ExternalDocumentation.cjs"));
var _Parameter = _interopRequireDefault(require("../../elements/Parameter.cjs"));
var _Items = _interopRequireDefault(require("../../elements/Items.cjs"));
var _Responses = _interopRequireDefault(require("../../elements/Responses.cjs"));
var _Response = _interopRequireDefault(require("../../elements/Response.cjs"));
var _Headers = _interopRequireDefault(require("../../elements/Headers.cjs"));
var _Example = _interopRequireDefault(require("../../elements/Example.cjs"));
var _Header = _interopRequireDefault(require("../../elements/Header.cjs"));
var _Tag = _interopRequireDefault(require("../../elements/Tag.cjs"));
var _Schema = _interopRequireDefault(require("../../elements/Schema.cjs"));
var _Xml = _interopRequireDefault(require("../../elements/Xml.cjs"));
var _Definitions = _interopRequireDefault(require("../../elements/Definitions.cjs"));
var _ParametersDefinitions = _interopRequireDefault(require("../../elements/ParametersDefinitions.cjs"));
var _ResponsesDefinitions = _interopRequireDefault(require("../../elements/ResponsesDefinitions.cjs"));
var _SecurityDefinitions = _interopRequireDefault(require("../../elements/SecurityDefinitions.cjs"));
var _SecurityScheme = _interopRequireDefault(require("../../elements/SecurityScheme.cjs"));
var _Scopes = _interopRequireDefault(require("../../elements/Scopes.cjs"));
var _SecurityRequirement = _interopRequireDefault(require("../../elements/SecurityRequirement.cjs"));
var _OperationConsumes = _interopRequireDefault(require("../../elements/nces/OperationConsumes.cjs"));
var _OperationParameters = _interopRequireDefault(require("../../elements/nces/OperationParameters.cjs"));
var _OperationProduces = _interopRequireDefault(require("../../elements/nces/OperationProduces.cjs"));
var _OperationSchemes = _interopRequireDefault(require("../../elements/nces/OperationSchemes.cjs"));
var _OperationSecurity = _interopRequireDefault(require("../../elements/nces/OperationSecurity.cjs"));
var _OperationTags = _interopRequireDefault(require("../../elements/nces/OperationTags.cjs"));
var _PathItemParameters = _interopRequireDefault(require("../../elements/nces/PathItemParameters.cjs"));
var _SwaggerSchemes = _interopRequireDefault(require("../../elements/nces/SwaggerSchemes.cjs"));
var _SwaggerConsumes = _interopRequireDefault(require("../../elements/nces/SwaggerConsumes.cjs"));
var _SwaggerProduces = _interopRequireDefault(require("../../elements/nces/SwaggerProduces.cjs"));
var _SwaggerSecurity = _interopRequireDefault(require("../../elements/nces/SwaggerSecurity.cjs"));
var _SwaggerTags = _interopRequireDefault(require("../../elements/nces/SwaggerTags.cjs"));
var _visitor = require("../../traversal/visitor.cjs");
/**
 * OpenAPI 2.0 specification elements.
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

const isEmptyElement = element => (0, _apidomCore.isStringElement)(element) && (0, _apidomCore.includesClasses)(['yaml-e-node', 'yaml-e-scalar'], element);
const schema = {
  // concrete types handling (CTs)
  SwaggerElement: {
    info(...args) {
      return new _Info.default(...args);
    },
    schemes(...args) {
      return new _SwaggerSchemes.default(...args);
    },
    consumes(...args) {
      return new _SwaggerConsumes.default(...args);
    },
    produces(...args) {
      return new _SwaggerProduces.default(...args);
    },
    paths(...args) {
      return new _Paths.default(...args);
    },
    definitions(...args) {
      return new _Definitions.default(...args);
    },
    parameters(...args) {
      return new _ParametersDefinitions.default(...args);
    },
    responses(...args) {
      return new _ResponsesDefinitions.default(...args);
    },
    securityDefinitions(...args) {
      return new _SecurityDefinitions.default(...args);
    },
    security(...args) {
      return new _SwaggerSecurity.default(...args);
    },
    tags(...args) {
      return new _SwaggerTags.default(...args);
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
    consumes(...args) {
      return new _OperationConsumes.default(...args);
    },
    produces(...args) {
      return new _OperationProduces.default(...args);
    },
    parameters(...args) {
      return new _OperationParameters.default(...args);
    },
    responses(...args) {
      return new _Responses.default(...args);
    },
    schemes(...args) {
      return new _OperationSchemes.default(...args);
    },
    security(...args) {
      return new _OperationSecurity.default(...args);
    }
  },
  ParameterElement: {
    schema(...args) {
      return new _Schema.default(...args);
    },
    items(...args) {
      return new _Items.default(...args);
    }
  },
  ItemsElement: {
    items(...args) {
      return new _Items.default(...args);
    }
  },
  ResponsesElement: {
    '[key: *]': function key(...args) {
      return new _Response.default(...args);
    }
  },
  ResponseElement: {
    schema(...args) {
      return new _Schema.default(...args);
    },
    headers(...args) {
      return new _Headers.default(...args);
    },
    examples(...args) {
      return new _Example.default(...args);
    }
  },
  HeadersElement: {
    '[key: *]': function key(...args) {
      return new _Header.default(...args);
    }
  },
  HeaderElement: {
    items(...args) {
      return new _Items.default(...args);
    }
  },
  TagElement: {
    externalDocs(...args) {
      return new _ExternalDocumentation.default(...args);
    }
  },
  SchemaElement: {
    items(...args) {
      return new _Schema.default(...args);
    },
    allOf(...args) {
      const element = new _apidomCore.ArrayElement(...args);
      element.classes.push('json-schema-allOf');
      return element;
    },
    properties(...args) {
      const element = new _apidomCore.ObjectElement(...args);
      element.classes.push('json-schema-properties');
      return element;
    },
    additionalProperties(...args) {
      return new _Schema.default(...args);
    },
    xml(...args) {
      return new _Xml.default(...args);
    },
    externalDocs(...args) {
      return new _ExternalDocumentation.default(...args);
    }
  },
  DefinitionsElement: {
    '[key: *]': function key(...args) {
      return new _Schema.default(...args);
    }
  },
  ParametersDefinitionsElement: {
    '[key: *]': function key(...args) {
      return new _Parameter.default(...args);
    }
  },
  ResponsesDefinitionsElement: {
    '[key: *]': function key(...args) {
      return new _Response.default(...args);
    }
  },
  SecurityDefinitionsElement: {
    '[key: *]': function key(...args) {
      return new _SecurityScheme.default(...args);
    }
  },
  SecuritySchemeElement: {
    scopes(...args) {
      return new _Scopes.default(...args);
    }
  },
  SecurityRequirementElement: {
    '[key: *]': function key(...args) {
      return new _apidomCore.ArrayElement(...args);
    }
  },
  // non-concrete types handling (NCEs)
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
  [_PathItemParameters.default.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new _Parameter.default(...args);
    }
  },
  [_SwaggerSecurity.default.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new _SecurityRequirement.default(...args);
    }
  },
  [_SwaggerTags.default.primaryClass]: {
    '<*>': function asterisk(...args) {
      return new _Tag.default(...args);
    }
  },
  'json-schema-properties': {
    '[key: *]': function key(...args) {
      return new _Schema.default(...args);
    }
  },
  'json-schema-allOf': {
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