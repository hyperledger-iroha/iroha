const JSONSerialiser = require('./JSONSerialiser');

module.exports = class JSON06Serialiser extends JSONSerialiser {
  serialise(element) {
    if (!(element instanceof this.namespace.elements.Element)) {
      throw new TypeError(`Given element \`${element}\` is not an Element instance`);
    }

    let variable;
    if (element._attributes && element.attributes.get('variable')) {
      variable = element.attributes.get('variable');
    }

    const payload = {
      element: element.element,
    };

    if (element._meta && element._meta.length > 0) {
      payload.meta = this.serialiseObject(element.meta);
    }

    const isEnum = (element.element === 'enum' || element.attributes.keys().indexOf('enumerations') !== -1);

    if (isEnum) {
      const attributes = this.enumSerialiseAttributes(element);

      if (attributes) {
        payload.attributes = attributes;
      }
    } else if (element._attributes && element._attributes.length > 0) {
      let { attributes } = element;

      // Meta attribute was renamed to metadata
      if (attributes.get('metadata')) {
        attributes = attributes.clone();
        attributes.set('meta', attributes.get('metadata'));
        attributes.remove('metadata');
      }

      if (element.element === 'member' && variable) {
        attributes = attributes.clone();
        attributes.remove('variable');
      }

      if (attributes.length > 0) {
        payload.attributes = this.serialiseObject(attributes);
      }
    }

    if (isEnum) {
      payload.content = this.enumSerialiseContent(element, payload);
    } else if (this[`${element.element}SerialiseContent`]) {
      payload.content = this[`${element.element}SerialiseContent`](element, payload);
    } else if (element.content !== undefined) {
      let content;

      if (variable && element.content.key) {
        content = element.content.clone();
        content.key.attributes.set('variable', variable);
        content = this.serialiseContent(content);
      } else {
        content = this.serialiseContent(element.content);
      }

      if (this.shouldSerialiseContent(element, content)) {
        payload.content = content;
      }
    } else if (this.shouldSerialiseContent(element, element.content) && element instanceof this.namespace.elements.Array) {
      payload.content = [];
    }

    return payload;
  }

  shouldSerialiseContent(element, content) {
    if (element.element === 'parseResult' || element.element === 'httpRequest'
        || element.element === 'httpResponse' || element.element === 'category'
        || element.element === 'link') {
      return true;
    }

    if (content === undefined) {
      return false;
    }

    if (Array.isArray(content) && content.length === 0) {
      return false;
    }

    return true;
  }

  refSerialiseContent(element, payload) {
    delete payload.attributes;

    return {
      href: element.toValue(),
      path: element.path.toValue(),
    };
  }

  sourceMapSerialiseContent(element) {
    return element.toValue();
  }

  dataStructureSerialiseContent(element) {
    return [this.serialiseContent(element.content)];
  }

  enumSerialiseAttributes(element) {
    const attributes = element.attributes.clone();

    // Enumerations attribute was is placed inside content (see `enumSerialiseContent` below)
    const enumerations = attributes.remove('enumerations') || new this.namespace.elements.Array([]);

    // Remove fixed type attribute from samples and default
    const defaultValue = attributes.get('default');
    let samples = attributes.get('samples') || new this.namespace.elements.Array([]);

    if (defaultValue && defaultValue.content) {
      if (defaultValue.content.attributes) {
        defaultValue.content.attributes.remove('typeAttributes');
      }
      // Wrap default in array (not sure it is really needed because tests pass without this line)
      attributes.set('default', new this.namespace.elements.Array([defaultValue.content]));
    }

    // Strip typeAttributes from samples, 0.6 doesn't usually contain them in samples
    samples.forEach((sample) => {
      if (sample.content && sample.content.element) {
        sample.content.attributes.remove('typeAttributes');
      }
    });

    // Content -> Samples
    if (element.content && enumerations.length !== 0) {
      // If we don't have enumerations, content should stay in
      // content (enumerations) as per Drafter 3 behaviour.
      samples.unshift(element.content);
    }

    samples = samples.map((sample) => {
      if (sample instanceof this.namespace.elements.Array) {
        return [sample];
      }

      return new this.namespace.elements.Array([sample.content]);
    });

    if (samples.length) {
      attributes.set('samples', samples);
    }

    if (attributes.length > 0) {
      return this.serialiseObject(attributes);
    }

    return undefined;
  }

  enumSerialiseContent(element) {
    // In API Elements < 1.0, the content is the enumerations
    // If we don't have an enumerations, use the value (Drafter 3 behaviour)

    if (element._attributes) {
      const enumerations = element.attributes.get('enumerations');

      if (enumerations && enumerations.length > 0) {
        return enumerations.content.map((enumeration) => {
          const e = enumeration.clone();
          e.attributes.remove('typeAttributes');
          return this.serialise(e);
        });
      }
    }

    if (element.content) {
      const value = element.content.clone();
      value.attributes.remove('typeAttributes');
      return [this.serialise(value)];
    }

    return [];
  }

  deserialise(value) {
    if (typeof value === 'string') {
      return new this.namespace.elements.String(value);
    }

    if (typeof value === 'number') {
      return new this.namespace.elements.Number(value);
    }

    if (typeof value === 'boolean') {
      return new this.namespace.elements.Boolean(value);
    }

    if (value === null) {
      return new this.namespace.elements.Null();
    }

    if (Array.isArray(value)) {
      return new this.namespace.elements.Array(value.map(this.deserialise, this));
    }

    const ElementClass = this.namespace.getElementClass(value.element);
    const element = new ElementClass();

    if (element.element !== value.element) {
      element.element = value.element;
    }

    if (value.meta) {
      this.deserialiseObject(value.meta, element.meta);
    }

    if (value.attributes) {
      this.deserialiseObject(value.attributes, element.attributes);
    }

    const content = this.deserialiseContent(value.content);
    if (content !== undefined || element.content === null) {
      element.content = content;
    }

    if (element.element === 'enum') {
      // Grab enumerations from content
      if (element.content) {
        element.attributes.set('enumerations', element.content);
      }

      // Unwrap the sample value (inside double array)
      let samples = element.attributes.get('samples');
      element.attributes.remove('samples');

      if (samples) {
        // Re-wrap samples from array of array to array of enum's

        const existingSamples = samples;

        samples = new this.namespace.elements.Array();
        existingSamples.forEach((existingSample) => {
          existingSample.forEach((sample) => {
            const enumElement = new ElementClass(sample);
            enumElement.element = element.element;
            samples.push(enumElement);
          });
        });

        const sample = samples.shift();

        if (sample) {
          element.content = sample.content;
        } else {
          element.content = undefined;
        }

        element.attributes.set('samples', samples);
      } else {
        element.content = undefined;
      }

      // Unwrap the default value
      let defaultValue = element.attributes.get('default');
      if (defaultValue && defaultValue.length > 0) {
        defaultValue = defaultValue.get(0);
        const defaultElement = new ElementClass(defaultValue);
        defaultElement.element = element.element;
        element.attributes.set('default', defaultElement);
      }
    } else if (element.element === 'dataStructure' && Array.isArray(element.content)) {
      [element.content] = element.content;
    } else if (element.element === 'category') {
      // "meta" attribute has been renamed to metadata
      const metadata = element.attributes.get('meta');

      if (metadata) {
        element.attributes.set('metadata', metadata);
        element.attributes.remove('meta');
      }
    } else if (element.element === 'member' && element.key && element.key._attributes && element.key._attributes.getValue('variable')) {
      element.attributes.set('variable', element.key.attributes.get('variable'));
      element.key.attributes.remove('variable');
    }

    return element;
  }

  // Private API

  serialiseContent(content) {
    if (content instanceof this.namespace.elements.Element) {
      return this.serialise(content);
    }

    if (content instanceof this.namespace.KeyValuePair) {
      const pair = {
        key: this.serialise(content.key),
      };

      if (content.value) {
        pair.value = this.serialise(content.value);
      }

      return pair;
    }

    if (content && content.map) {
      return content.map(this.serialise, this);
    }

    return content;
  }

  deserialiseContent(content) {
    if (content) {
      if (content.element) {
        return this.deserialise(content);
      }

      if (content.key) {
        const pair = new this.namespace.KeyValuePair(this.deserialise(content.key));

        if (content.value) {
          pair.value = this.deserialise(content.value);
        }

        return pair;
      }

      if (content.map) {
        return content.map(this.deserialise, this);
      }
    }

    return content;
  }

  shouldRefract(element) {
    if ((element._attributes && element.attributes.keys().length) || (element._meta && element.meta.keys().length)) {
      return true;
    }

    if (element.element === 'enum') {
      // enum elements are treated like primitives (array)
      return false;
    }

    if (element.element !== element.primitive() || element.element === 'member') {
      return true;
    }

    return false;
  }

  convertKeyToRefract(key, item) {
    if (this.shouldRefract(item)) {
      return this.serialise(item);
    }

    if (item.element === 'enum') {
      return this.serialiseEnum(item);
    }

    if (item.element === 'array') {
      return item.map((subItem) => {
        if (this.shouldRefract(subItem) || key === 'default') {
          return this.serialise(subItem);
        }

        if (subItem.element === 'array' || subItem.element === 'object' || subItem.element === 'enum') {
          // items for array or enum inside array are always serialised
          return subItem.children.map(subSubItem => this.serialise(subSubItem));
        }

        return subItem.toValue();
      });
    }

    if (item.element === 'object') {
      return (item.content || []).map(this.serialise, this);
    }

    return item.toValue();
  }

  serialiseEnum(element) {
    return element.children.map(item => this.serialise(item));
  }

  serialiseObject(obj) {
    const result = {};

    obj.forEach((value, key) => {
      if (value) {
        const keyValue = key.toValue();
        result[keyValue] = this.convertKeyToRefract(keyValue, value);
      }
    });

    return result;
  }

  deserialiseObject(from, to) {
    Object.keys(from).forEach((key) => {
      to.set(key, this.deserialise(from[key]));
    });
  }
};
