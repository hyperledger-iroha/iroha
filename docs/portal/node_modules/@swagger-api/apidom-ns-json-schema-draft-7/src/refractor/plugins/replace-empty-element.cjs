"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _JSONSchema = _interopRequireDefault(require("../../elements/JSONSchema.cjs"));
var _LinkDescription = _interopRequireDefault(require("../../elements/LinkDescription.cjs"));
var _visitor = require("../../traversal/visitor.cjs");
/**
 * JSON Schema Draft 7 specification elements.
 */

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
 * $schema: http://json-schema.org/draft-07/schema#
 * items:
 * ```
 * Refracting result without this plugin:
 *
 *  (JSONSchemaElement
 *    (MemberElement
 *      (StringElement)
 *      (StringElement))
 *    (MemberElement
 *      (StringElement)
 *      (StringElement))
 *
 * Refracting result with this plugin:
 *
 *  (JSONSchemaElement
 *    (MemberElement
 *      (StringElement)
 *      (StringElement))
 *    (MemberElement
 *      (StringElement)
 *      (JSONSchemaElement))
 */

const isEmptyElement = element => (0, _apidomCore.isStringElement)(element) && (0, _apidomCore.includesClasses)(['yaml-e-node', 'yaml-e-scalar'], element);
const schema = {
  JSONSchemaDraft7Element: {
    additionalItems(...args) {
      return new _JSONSchema.default(...args);
    },
    items(...args) {
      return new _JSONSchema.default(...args);
    },
    contains(...args) {
      return new _JSONSchema.default(...args);
    },
    required(...args) {
      const element = new _apidomCore.ArrayElement(...args);
      element.classes.push('json-schema-required');
      return element;
    },
    properties(...args) {
      const element = new _apidomCore.ObjectElement(...args);
      element.classes.push('json-schema-properties');
      return element;
    },
    additionalProperties(...args) {
      return new _JSONSchema.default(...args);
    },
    patternProperties(...args) {
      const element = new _apidomCore.ObjectElement(...args);
      element.classes.push('json-schema-patternProperties');
      return element;
    },
    dependencies(...args) {
      const element = new _apidomCore.ObjectElement(...args);
      element.classes.push('json-schema-dependencies');
      return element;
    },
    propertyNames(...args) {
      return new _JSONSchema.default(...args);
    },
    enum(...args) {
      const element = new _apidomCore.ArrayElement(...args);
      element.classes.push('json-schema-enum');
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
    if(...args) {
      return new _JSONSchema.default(...args);
    },
    then(...args) {
      return new _JSONSchema.default(...args);
    },
    else(...args) {
      return new _JSONSchema.default(...args);
    },
    not(...args) {
      return new _JSONSchema.default(...args);
    },
    definitions(...args) {
      const element = new _apidomCore.ObjectElement(...args);
      element.classes.push('json-schema-definitions');
      return element;
    },
    examples(...args) {
      const element = new _apidomCore.ArrayElement(...args);
      element.classes.push('json-schema-examples');
      return element;
    },
    links(...args) {
      const element = new _apidomCore.ArrayElement(...args);
      element.classes.push('json-schema-links');
      return element;
    }
  },
  LinkDescriptionElement: {
    hrefSchema(...args) {
      return new _JSONSchema.default(...args);
    },
    targetSchema(...args) {
      return new _JSONSchema.default(...args);
    },
    submissionSchema(...args) {
      return new _JSONSchema.default(...args);
    },
    templatePointers(...args) {
      return new _apidomCore.ObjectElement(...args);
    },
    templateRequired(...args) {
      return new _apidomCore.ArrayElement(...args);
    },
    targetHints(...args) {
      return new _apidomCore.ObjectElement(...args);
    },
    headerSchema(...args) {
      return new _JSONSchema.default(...args);
    }
  },
  'json-schema-properties': {
    '[key: *]': function key(...args) {
      return new _JSONSchema.default(...args);
    }
  },
  'json-schema-patternProperties': {
    '[key: *]': function key(...args) {
      return new _JSONSchema.default(...args);
    }
  },
  'json-schema-dependencies': {
    '[key: *]': function key(...args) {
      return new _JSONSchema.default(...args);
    }
  },
  'json-schema-allOf': {
    '<*>': function asterisk(...args) {
      return new _JSONSchema.default(...args);
    }
  },
  'json-schema-anyOf': {
    '<*>': function asterisk(...args) {
      return new _JSONSchema.default(...args);
    }
  },
  'json-schema-oneOf': {
    '<*>': function asterisk(...args) {
      return new _JSONSchema.default(...args);
    }
  },
  'json-schema-definitions': {
    '[key: *]': function key(...args) {
      return new _JSONSchema.default(...args);
    }
  },
  'json-schema-links': {
    '<*>': function asterisk(...args) {
      return new _LinkDescription.default(...args);
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
const plugin = () => () => {
  return {
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
  };
};
var _default = exports.default = plugin;