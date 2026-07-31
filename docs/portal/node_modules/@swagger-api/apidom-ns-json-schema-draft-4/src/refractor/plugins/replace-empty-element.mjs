import { ArrayElement, ObjectElement, isArrayElement, isElement, isMemberElement, isStringElement, includesClasses, cloneDeep, toValue, assignSourceMap } from '@swagger-api/apidom-core';

/**
 * JSON Schema Draft 4 specification elements.
 */
import JSONSchemaElement from "../../elements/JSONSchema.mjs";
import LinkDescriptionElement from "../../elements/LinkDescription.mjs";
import MediaElement from "../../elements/Media.mjs";
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
 * $schema: http://json-schema.org/draft-04/schema#
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
const isEmptyElement = element => isStringElement(element) && includesClasses(['yaml-e-node', 'yaml-e-scalar'], element);
const schema = {
  JSONSchemaDraft4Element: {
    additionalItems(...args) {
      return new JSONSchemaElement(...args);
    },
    items(...args) {
      return new JSONSchemaElement(...args);
    },
    required(...args) {
      const element = new ArrayElement(...args);
      element.classes.push('json-schema-required');
      return element;
    },
    properties(...args) {
      const element = new ObjectElement(...args);
      element.classes.push('json-schema-properties');
      return element;
    },
    additionalProperties(...args) {
      return new JSONSchemaElement(...args);
    },
    patternProperties(...args) {
      const element = new ObjectElement(...args);
      element.classes.push('json-schema-patternProperties');
      return element;
    },
    dependencies(...args) {
      const element = new ObjectElement(...args);
      element.classes.push('json-schema-dependencies');
      return element;
    },
    enum(...args) {
      const element = new ArrayElement(...args);
      element.classes.push('json-schema-enum');
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
      return new JSONSchemaElement(...args);
    },
    definitions(...args) {
      const element = new ObjectElement(...args);
      element.classes.push('json-schema-definitions');
      return element;
    },
    links(...args) {
      const element = new ArrayElement(...args);
      element.classes.push('json-schema-links');
      return element;
    },
    media(...args) {
      return new MediaElement(...args);
    }
  },
  LinkDescriptionElement: {
    targetSchema(...args) {
      return new JSONSchemaElement(...args);
    },
    schema(...args) {
      return new JSONSchemaElement(...args);
    }
  },
  'json-schema-properties': {
    '[key: *]': function key(...args) {
      return new JSONSchemaElement(...args);
    }
  },
  'json-schema-patternProperties': {
    '[key: *]': function key(...args) {
      return new JSONSchemaElement(...args);
    }
  },
  'json-schema-dependencies': {
    '[key: *]': function key(...args) {
      return new JSONSchemaElement(...args);
    }
  },
  'json-schema-allOf': {
    '<*>': function asterisk(...args) {
      return new JSONSchemaElement(...args);
    }
  },
  'json-schema-anyOf': {
    '<*>': function asterisk(...args) {
      return new JSONSchemaElement(...args);
    }
  },
  'json-schema-oneOf': {
    '<*>': function asterisk(...args) {
      return new JSONSchemaElement(...args);
    }
  },
  'json-schema-definitions': {
    '[key: *]': function key(...args) {
      return new JSONSchemaElement(...args);
    }
  },
  'json-schema-links': {
    '<*>': function asterisk(...args) {
      return new LinkDescriptionElement(...args);
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