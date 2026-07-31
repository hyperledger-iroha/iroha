import { ArrayElement, ObjectElement, isStringElement, isArrayElement, isElement, isMemberElement, includesClasses, cloneDeep, toValue, assignSourceMap } from '@swagger-api/apidom-core';
/**
 * JSON Schema 2020-12 specification elements.
 */
import JSONSchemaElement from "../../elements/JSONSchema.mjs";
import LinkDescriptionElement from "../../elements/LinkDescription.mjs";
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
 * $schema: https://json-schema.org/draft/2020-12/schema
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
  JSONSchema202012Element: {
    prefixItems(...args) {
      const element = new ArrayElement(...args);
      element.classes.push('json-schema-prefixItems');
      return element;
    },
    items(...args) {
      return new JSONSchemaElement(...args);
    },
    contains(...args) {
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
    dependentSchemas(...args) {
      const element = new ObjectElement(...args);
      element.classes.push('json-schema-dependentSchemas');
      return element;
    },
    propertyNames(...args) {
      return new JSONSchemaElement(...args);
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
    if(...args) {
      return new JSONSchemaElement(...args);
    },
    then(...args) {
      return new JSONSchemaElement(...args);
    },
    else(...args) {
      return new JSONSchemaElement(...args);
    },
    not(...args) {
      return new JSONSchemaElement(...args);
    },
    $defs(...args) {
      const element = new ObjectElement(...args);
      element.classes.push('json-schema-$defs');
      return element;
    },
    examples(...args) {
      const element = new ArrayElement(...args);
      element.classes.push('json-schema-examples');
      return element;
    },
    links(...args) {
      const element = new ArrayElement(...args);
      element.classes.push('json-schema-links');
      return element;
    },
    $vocabulary(...args) {
      const element = new ObjectElement(...args);
      element.classes.push('json-schema-$vocabulary');
      return element;
    },
    unevaluatedItems(...args) {
      return new JSONSchemaElement(...args);
    },
    unevaluatedProperties(...args) {
      return new JSONSchemaElement(...args);
    },
    $dependentRequired(...args) {
      const element = new ObjectElement(...args);
      element.classes.push('json-schema-$dependentRequired');
      return element;
    },
    contentSchema(...args) {
      return new JSONSchemaElement(...args);
    },
    type(...args) {
      const element = new ArrayElement(...args);
      element.classes.push('json-schema-type');
      return element;
    }
  },
  LinkDescriptionElement: {
    hrefSchema(...args) {
      return new JSONSchemaElement(...args);
    },
    targetSchema(...args) {
      return new JSONSchemaElement(...args);
    },
    submissionSchema(...args) {
      return new JSONSchemaElement(...args);
    },
    templatePointers(...args) {
      return new ObjectElement(...args);
    },
    templateRequired(...args) {
      return new ArrayElement(...args);
    },
    targetHints(...args) {
      return new ObjectElement(...args);
    },
    headerSchema(...args) {
      return new JSONSchemaElement(...args);
    }
  },
  'json-schema-prefixItems': {
    '<*>': function asterisk(...args) {
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
  'json-schema-dependentSchemas': {
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
  'json-schema-$defs': {
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
const plugin = () => () => {
  return {
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
  };
};
export default plugin;