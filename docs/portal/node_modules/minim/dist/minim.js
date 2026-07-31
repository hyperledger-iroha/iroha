(function(f){if(typeof exports==="object"&&typeof module!=="undefined"){module.exports=f()}else if(typeof define==="function"&&define.amd){define([],f)}else{var g;if(typeof window!=="undefined"){g=window}else if(typeof global!=="undefined"){g=global}else if(typeof self!=="undefined"){g=self}else{g=this}g.minim = f()}})(function(){var define,module,exports;return (function(){function r(e,n,t){function o(i,f){if(!n[i]){if(!e[i]){var c="function"==typeof require&&require;if(!f&&c)return c(i,!0);if(u)return u(i,!0);var a=new Error("Cannot find module '"+i+"'");throw a.code="MODULE_NOT_FOUND",a}var p=n[i]={exports:{}};e[i][0].call(p.exports,function(r){var n=e[i][1][r];return o(n||r)},p,p.exports,r,e,n,t)}return n[i].exports}for(var u="function"==typeof require&&require,i=0;i<t.length;i++)o(t[i]);return o}return r})()({1:[function(require,module,exports){
'use strict';

var _createClass = function () {
  function defineProperties(target, props) {
    for (var i = 0; i < props.length; i++) {
      var descriptor = props[i];descriptor.enumerable = descriptor.enumerable || false;descriptor.configurable = true;if ("value" in descriptor) descriptor.writable = true;Object.defineProperty(target, descriptor.key, descriptor);
    }
  }return function (Constructor, protoProps, staticProps) {
    if (protoProps) defineProperties(Constructor.prototype, protoProps);if (staticProps) defineProperties(Constructor, staticProps);return Constructor;
  };
}();

function _classCallCheck(instance, Constructor) {
  if (!(instance instanceof Constructor)) {
    throw new TypeError("Cannot call a class as a function");
  }
}

var negate = require('lodash/negate');

// Coerces an a parameter into a callback for matching elements.
// This accepts an element name, an element type and returns a
// callback to match for those elements.
function coerceElementMatchingCallback(value) {
  // Element Name
  if (typeof value === 'string') {
    return function (element) {
      return element.element === value;
    };
  }

  // Element Type
  if (value.constructor && value.extend) {
    return function (element) {
      return element instanceof value;
    };
  }

  return value;
}

/**
 * @class
 *
 * @param {Element[]} elements
 *
 * @property {Element[]} elements
 */

var ArraySlice = function () {
  function ArraySlice(elements) {
    _classCallCheck(this, ArraySlice);

    this.elements = elements || [];
  }

  /**
   * @returns {Array}
   */

  _createClass(ArraySlice, [{
    key: 'toValue',
    value: function toValue() {
      return this.elements.map(function (element) {
        return element.toValue();
      });
    }

    // High Order Functions

    /**
     * @param callback - Function to execute for each element
     * @param thisArg - Value to use as this (i.e the reference Object) when executing callback
     * @returns {array} A new array with each element being the result of the callback function
     */

  }, {
    key: 'map',
    value: function map(callback, thisArg) {
      return this.elements.map(callback, thisArg);
    }

    /**
     * Maps and then flattens the results.
     * @param callback - Function to execute for each element.
     * @param thisArg - Value to use as this (i.e the reference Object) when executing callback
     * @returns {array}
     */

  }, {
    key: 'flatMap',
    value: function flatMap(callback, thisArg) {
      return this.map(callback, thisArg).reduce(function (a, b) {
        return a.concat(b);
      }, []);
    }

    /**
     * Returns an array containing the truthy results of calling the given transformation with each element of this sequence
     * @param transform - A closure that accepts an element of this array as its argument and returns an optional value.
     * @param thisArg - Value to use as this (i.e the reference Object) when executing callback
     * @memberof ArraySlice.prototype
     * @returns An array of the non-undefined results of calling transform with each element of the array
     */

  }, {
    key: 'compactMap',
    value: function compactMap(transform, thisArg) {
      var results = [];

      this.forEach(function (element) {
        var result = transform.bind(thisArg)(element);

        if (result) {
          results.push(result);
        }
      });

      return results;
    }

    /**
     * @param callback - Function to execute for each element. This may be a callback, an element name or an element class.
     * @param thisArg - Value to use as this (i.e the reference Object) when executing callback
     * @returns {ArraySlice}
     * @memberof ArraySlice.prototype
     */

  }, {
    key: 'filter',
    value: function filter(callback, thisArg) {
      callback = coerceElementMatchingCallback(callback);
      return new ArraySlice(this.elements.filter(callback, thisArg));
    }

    /**
     * @param callback - Function to execute for each element. This may be a callback, an element name or an element class.
     * @param thisArg - Value to use as this (i.e the reference Object) when executing callback
     * @returns {ArraySlice}
     * @memberof ArraySlice.prototype
     */

  }, {
    key: 'reject',
    value: function reject(callback, thisArg) {
      callback = coerceElementMatchingCallback(callback);
      return new ArraySlice(this.elements.filter(negate(callback), thisArg));
    }

    /**
     * Returns the first element in the array that satisfies the given value
     * @param callback - Function to execute for each element. This may be a callback, an element name or an element class.
     * @param thisArg - Value to use as this (i.e the reference Object) when executing callback
     * @returns {Element}
     * @memberof ArraySlice.prototype
     */

  }, {
    key: 'find',
    value: function find(callback, thisArg) {
      callback = coerceElementMatchingCallback(callback);
      return this.elements.find(callback, thisArg);
    }

    /**
     * @param callback - Function to execute for each element
     * @param thisArg - Value to use as this (i.e the reference Object) when executing callback
     * @memberof ArraySlice.prototype
     */

  }, {
    key: 'forEach',
    value: function forEach(callback, thisArg) {
      this.elements.forEach(callback, thisArg);
    }

    /**
     * @param callback - Function to execute for each element
     * @param initialValue
     * @memberof ArraySlice.prototype
     */

  }, {
    key: 'reduce',
    value: function reduce(callback, initialValue) {
      return this.elements.reduce(callback, initialValue);
    }

    /**
     * @param value
     * @returns {boolean}
     * @memberof ArraySlice.prototype
     */

  }, {
    key: 'includes',
    value: function includes(value) {
      return this.elements.some(function (element) {
        return element.equals(value);
      });
    }

    // Mutation

    /**
     * Removes the first element from the slice
     * @returns {Element} The removed element or undefined if the slice is empty
     * @memberof ArraySlice.prototype
     */

  }, {
    key: 'shift',
    value: function shift() {
      return this.elements.shift();
    }

    /**
     * Adds the given element to the begining of the slice
     * @parameter {Element} value
     * @memberof ArraySlice.prototype
     */

  }, {
    key: 'unshift',
    value: function unshift(value) {
      this.elements.unshift(this.refract(value));
    }

    /**
     * Adds the given element to the end of the slice
     * @parameter {Element} value
     * @memberof ArraySlice.prototype
     */

  }, {
    key: 'push',
    value: function push(value) {
      this.elements.push(this.refract(value));
      return this;
    }

    /**
     * @parameter {Element} value
     * @memberof ArraySlice.prototype
     */

  }, {
    key: 'add',
    value: function add(value) {
      this.push(value);
    }

    // Accessors

    /**
     * @parameter {number} index
     * @returns {Element}
     * @memberof ArraySlice.prototype
     */

  }, {
    key: 'get',
    value: function get(index) {
      return this.elements[index];
    }

    /**
     * @parameter {number} index
     * @memberof ArraySlice.prototype
     */

  }, {
    key: 'getValue',
    value: function getValue(index) {
      var element = this.elements[index];

      if (element) {
        return element.toValue();
      }

      return undefined;
    }

    /**
     * Returns the number of elements in the slice
     * @type number
     */

  }, {
    key: 'length',
    get: function get() {
      return this.elements.length;
    }

    /**
     * Returns whether the slice is empty
     * @type boolean
     */

  }, {
    key: 'isEmpty',
    get: function get() {
      return this.elements.length === 0;
    }

    /**
     * Returns the first element in the slice or undefined if the slice is empty
     * @type Element
     */

  }, {
    key: 'first',
    get: function get() {
      return this.elements[0];
    }
  }]);

  return ArraySlice;
}();

if (typeof Symbol !== 'undefined') {
  ArraySlice.prototype[Symbol.iterator] = function symbol() {
    return this.elements[Symbol.iterator]();
  };
}

module.exports = ArraySlice;

},{"lodash/negate":110}],2:[function(require,module,exports){
"use strict";

var _createClass = function () {
  function defineProperties(target, props) {
    for (var i = 0; i < props.length; i++) {
      var descriptor = props[i];descriptor.enumerable = descriptor.enumerable || false;descriptor.configurable = true;if ("value" in descriptor) descriptor.writable = true;Object.defineProperty(target, descriptor.key, descriptor);
    }
  }return function (Constructor, protoProps, staticProps) {
    if (protoProps) defineProperties(Constructor.prototype, protoProps);if (staticProps) defineProperties(Constructor, staticProps);return Constructor;
  };
}();

function _classCallCheck(instance, Constructor) {
  if (!(instance instanceof Constructor)) {
    throw new TypeError("Cannot call a class as a function");
  }
}

/**
 * @class
 *
 * @property {Element} key
 * @property {Element} value
 */
var KeyValuePair = function () {
  function KeyValuePair(key, value) {
    _classCallCheck(this, KeyValuePair);

    this.key = key;
    this.value = value;
  }

  /**
   * @returns {KeyValuePair}
   */

  _createClass(KeyValuePair, [{
    key: "clone",
    value: function clone() {
      var clone = new KeyValuePair();

      if (this.key) {
        clone.key = this.key.clone();
      }

      if (this.value) {
        clone.value = this.value.clone();
      }

      return clone;
    }
  }]);

  return KeyValuePair;
}();

module.exports = KeyValuePair;

},{}],3:[function(require,module,exports){
'use strict';

var _createClass = function () {
  function defineProperties(target, props) {
    for (var i = 0; i < props.length; i++) {
      var descriptor = props[i];descriptor.enumerable = descriptor.enumerable || false;descriptor.configurable = true;if ("value" in descriptor) descriptor.writable = true;Object.defineProperty(target, descriptor.key, descriptor);
    }
  }return function (Constructor, protoProps, staticProps) {
    if (protoProps) defineProperties(Constructor.prototype, protoProps);if (staticProps) defineProperties(Constructor, staticProps);return Constructor;
  };
}();

function _classCallCheck(instance, Constructor) {
  if (!(instance instanceof Constructor)) {
    throw new TypeError("Cannot call a class as a function");
  }
}

var isNull = require('lodash/isNull');
var isString = require('lodash/isString');
var isNumber = require('lodash/isNumber');
var isBoolean = require('lodash/isBoolean');
var isObject = require('lodash/isObject');

var JSONSerialiser = require('./serialisers/JSONSerialiser');
var elements = require('./elements');

/**
 * @class
 *
 * A refract element implementation with an extensible namespace, able to
 * load other namespaces into it.
 *
 * The namespace allows you to register your own classes to be instantiated
 * when a particular refract element is encountered, and allows you to specify
 * which elements get instantiated for existing Javascript objects.
 */

var Namespace = function () {
  function Namespace(options) {
    _classCallCheck(this, Namespace);

    this.elementMap = {};
    this.elementDetection = [];
    this.Element = elements.Element;
    this.KeyValuePair = elements.KeyValuePair;

    if (!options || !options.noDefault) {
      this.useDefault();
    }

    // These provide the defaults for new elements.
    this._attributeElementKeys = [];
    this._attributeElementArrayKeys = [];
  }

  /**
   * Use a namespace plugin or load a generic plugin.
   *
   * @param plugin
   */

  _createClass(Namespace, [{
    key: 'use',
    value: function use(plugin) {
      if (plugin.namespace) {
        plugin.namespace({ base: this });
      }
      if (plugin.load) {
        plugin.load({ base: this });
      }
      return this;
    }

    /*
     * Use the default namespace. This preloads all the default elements
     * into this registry instance.
     */

  }, {
    key: 'useDefault',
    value: function useDefault() {
      // Set up classes for default elements
      this.register('null', elements.NullElement).register('string', elements.StringElement).register('number', elements.NumberElement).register('boolean', elements.BooleanElement).register('array', elements.ArrayElement).register('object', elements.ObjectElement).register('member', elements.MemberElement).register('ref', elements.RefElement).register('link', elements.LinkElement);

      // Add instance detection functions to convert existing objects into
      // the corresponding refract elements.
      this.detect(isNull, elements.NullElement, false).detect(isString, elements.StringElement, false).detect(isNumber, elements.NumberElement, false).detect(isBoolean, elements.BooleanElement, false).detect(Array.isArray, elements.ArrayElement, false).detect(isObject, elements.ObjectElement, false);

      return this;
    }

    /**
     * Register a new element class for an element.
     *
     * @param {string} name
     * @param elementClass
     */

  }, {
    key: 'register',
    value: function register(name, ElementClass) {
      this._elements = undefined;
      this.elementMap[name] = ElementClass;
      return this;
    }

    /**
     * Unregister a previously registered class for an element.
     *
     * @param {string} name
     */

  }, {
    key: 'unregister',
    value: function unregister(name) {
      this._elements = undefined;
      delete this.elementMap[name];
      return this;
    }

    /*
     * Add a new detection function to determine which element
     * class to use when converting existing js instances into
     * refract element.
     */

  }, {
    key: 'detect',
    value: function detect(test, ElementClass, givenPrepend) {
      var prepend = givenPrepend === undefined ? true : givenPrepend;

      if (prepend) {
        this.elementDetection.unshift([test, ElementClass]);
      } else {
        this.elementDetection.push([test, ElementClass]);
      }

      return this;
    }

    /*
     * Convert an existing Javascript object into refract element instances, which
     * can be further processed or serialized into refract.
     * If the item passed in is already refracted, then it is returned
     * unmodified.
     */

  }, {
    key: 'toElement',
    value: function toElement(value) {
      if (value instanceof this.Element) {
        return value;
      }

      var element = void 0;

      for (var i = 0; i < this.elementDetection.length; i += 1) {
        var test = this.elementDetection[i][0];
        var ElementClass = this.elementDetection[i][1];

        if (test(value)) {
          element = new ElementClass(value);
          break;
        }
      }

      return element;
    }

    /*
     * Get an element class given an element name.
     */

  }, {
    key: 'getElementClass',
    value: function getElementClass(element) {
      var ElementClass = this.elementMap[element];

      if (ElementClass === undefined) {
        // Fall back to the base element. We may not know what
        // to do with the `content`, but downstream software
        // may know.
        return this.Element;
      }

      return ElementClass;
    }

    /*
     * Convert a refract document into refract element instances.
     */

  }, {
    key: 'fromRefract',
    value: function fromRefract(doc) {
      return this.serialiser.deserialise(doc);
    }

    /*
     * Convert an element to a Refracted JSON object.
     */

  }, {
    key: 'toRefract',
    value: function toRefract(element) {
      return this.serialiser.serialise(element);
    }

    /*
     * Get an object that contains all registered element classes, where
     * the key is the PascalCased element name and the value is the class.
     */

  }, {
    key: 'elements',
    get: function get() {
      var _this = this;

      if (this._elements === undefined) {
        this._elements = {
          Element: this.Element
        };

        Object.keys(this.elementMap).forEach(function (name) {
          // Currently, all registered element types use a camelCaseName.
          // Converting to PascalCase is as simple as upper-casing the first
          // letter.
          var pascal = name[0].toUpperCase() + name.substr(1);
          _this._elements[pascal] = _this.elementMap[name];
        });
      }

      return this._elements;
    }

    /**
     * Convinience method for getting a JSON Serialiser configured with the
     * current namespace
     *
     * @type JSONSerialiser
     * @readonly
     *
     * @memberof Namespace.prototype
     */

  }, {
    key: 'serialiser',
    get: function get() {
      return new JSONSerialiser(this);
    }
  }]);

  return Namespace;
}();

JSONSerialiser.prototype.Namespace = Namespace;

module.exports = Namespace;

},{"./elements":5,"./serialisers/JSONSerialiser":18,"lodash/isBoolean":98,"lodash/isNull":103,"lodash/isNumber":104,"lodash/isObject":105,"lodash/isString":107}],4:[function(require,module,exports){
'use strict';

var _typeof = typeof Symbol === "function" && typeof Symbol.iterator === "symbol" ? function (obj) { return typeof obj; } : function (obj) { return obj && typeof Symbol === "function" && obj.constructor === Symbol && obj !== Symbol.prototype ? "symbol" : typeof obj; };

var _createClass = function () {
  function defineProperties(target, props) {
    for (var i = 0; i < props.length; i++) {
      var descriptor = props[i];descriptor.enumerable = descriptor.enumerable || false;descriptor.configurable = true;if ("value" in descriptor) descriptor.writable = true;Object.defineProperty(target, descriptor.key, descriptor);
    }
  }return function (Constructor, protoProps, staticProps) {
    if (protoProps) defineProperties(Constructor.prototype, protoProps);if (staticProps) defineProperties(Constructor, staticProps);return Constructor;
  };
}();

function _classCallCheck(instance, Constructor) {
  if (!(instance instanceof Constructor)) {
    throw new TypeError("Cannot call a class as a function");
  }
}

function _possibleConstructorReturn(self, call) {
  if (!self) {
    throw new ReferenceError("this hasn't been initialised - super() hasn't been called");
  }return call && ((typeof call === "undefined" ? "undefined" : _typeof(call)) === "object" || typeof call === "function") ? call : self;
}

function _inherits(subClass, superClass) {
  if (typeof superClass !== "function" && superClass !== null) {
    throw new TypeError("Super expression must either be null or a function, not " + (typeof superClass === "undefined" ? "undefined" : _typeof(superClass)));
  }subClass.prototype = Object.create(superClass && superClass.prototype, { constructor: { value: subClass, enumerable: false, writable: true, configurable: true } });if (superClass) Object.setPrototypeOf ? Object.setPrototypeOf(subClass, superClass) : subClass.__proto__ = superClass;
}

var negate = require('lodash/negate');
var ArraySlice = require('./ArraySlice');

/**
 */

var ObjectSlice = function (_ArraySlice) {
  _inherits(ObjectSlice, _ArraySlice);

  function ObjectSlice() {
    _classCallCheck(this, ObjectSlice);

    return _possibleConstructorReturn(this, (ObjectSlice.__proto__ || Object.getPrototypeOf(ObjectSlice)).apply(this, arguments));
  }

  _createClass(ObjectSlice, [{
    key: 'map',
    value: function map(callback, thisArg) {
      return this.elements.map(function (member) {
        return callback.bind(thisArg)(member.value, member.key, member);
      });
    }
  }, {
    key: 'filter',
    value: function filter(callback, thisArg) {
      return new ObjectSlice(this.elements.filter(function (member) {
        return callback.bind(thisArg)(member.value, member.key, member);
      }));
    }
  }, {
    key: 'reject',
    value: function reject(callback, thisArg) {
      return this.filter(negate(callback.bind(thisArg)));
    }
  }, {
    key: 'forEach',
    value: function forEach(callback, thisArg) {
      return this.elements.forEach(function (member, index) {
        callback.bind(thisArg)(member.value, member.key, member, index);
      });
    }

    /**
     * @returns {array}
     */

  }, {
    key: 'keys',
    value: function keys() {
      return this.map(function (value, key) {
        return key.toValue();
      });
    }

    /**
     * @returns {array}
     */

  }, {
    key: 'values',
    value: function values() {
      return this.map(function (value) {
        return value.toValue();
      });
    }
  }]);

  return ObjectSlice;
}(ArraySlice);

module.exports = ObjectSlice;

},{"./ArraySlice":1,"lodash/negate":110}],5:[function(require,module,exports){
'use strict';

var _typeof2 = typeof Symbol === "function" && typeof Symbol.iterator === "symbol" ? function (obj) { return typeof obj; } : function (obj) { return obj && typeof Symbol === "function" && obj.constructor === Symbol && obj !== Symbol.prototype ? "symbol" : typeof obj; };

var _typeof = typeof Symbol === "function" && _typeof2(Symbol.iterator) === "symbol" ? function (obj) {
  return typeof obj === "undefined" ? "undefined" : _typeof2(obj);
} : function (obj) {
  return obj && typeof Symbol === "function" && obj.constructor === Symbol && obj !== Symbol.prototype ? "symbol" : typeof obj === "undefined" ? "undefined" : _typeof2(obj);
};

var Element = require('./primitives/Element');
var NullElement = require('./primitives/NullElement');
var StringElement = require('./primitives/StringElement');
var NumberElement = require('./primitives/NumberElement');
var BooleanElement = require('./primitives/BooleanElement');
var ArrayElement = require('./primitives/ArrayElement');
var MemberElement = require('./primitives/MemberElement');
var ObjectElement = require('./primitives/ObjectElement');
var LinkElement = require('./elements/LinkElement');
var RefElement = require('./elements/RefElement');

var ArraySlice = require('./ArraySlice');
var ObjectSlice = require('./ObjectSlice');

var KeyValuePair = require('./KeyValuePair');

/**
 * Refracts a JSON type to minim elements
 * @param value
 * @returns {Element}
 */
function refract(value) {
  if (value instanceof Element) {
    return value;
  }

  if (typeof value === 'string') {
    return new StringElement(value);
  }

  if (typeof value === 'number') {
    return new NumberElement(value);
  }

  if (typeof value === 'boolean') {
    return new BooleanElement(value);
  }

  if (value === null) {
    return new NullElement();
  }

  if (Array.isArray(value)) {
    return new ArrayElement(value.map(refract));
  }

  if ((typeof value === 'undefined' ? 'undefined' : _typeof(value)) === 'object') {
    var element = new ObjectElement(value);
    return element;
  }

  return value;
}

Element.prototype.ObjectElement = ObjectElement;
Element.prototype.RefElement = RefElement;
Element.prototype.MemberElement = MemberElement;

Element.prototype.refract = refract;
ArraySlice.prototype.refract = refract;

/**
 * Contains all of the element classes, and related structures and methods
 * for handling with element instances.
 */
module.exports = {
  Element: Element,
  NullElement: NullElement,
  StringElement: StringElement,
  NumberElement: NumberElement,
  BooleanElement: BooleanElement,
  ArrayElement: ArrayElement,
  MemberElement: MemberElement,
  ObjectElement: ObjectElement,
  LinkElement: LinkElement,
  RefElement: RefElement,

  refract: refract,

  ArraySlice: ArraySlice,
  ObjectSlice: ObjectSlice,
  KeyValuePair: KeyValuePair
};

},{"./ArraySlice":1,"./KeyValuePair":2,"./ObjectSlice":4,"./elements/LinkElement":6,"./elements/RefElement":7,"./primitives/ArrayElement":9,"./primitives/BooleanElement":10,"./primitives/Element":11,"./primitives/MemberElement":12,"./primitives/NullElement":13,"./primitives/NumberElement":14,"./primitives/ObjectElement":15,"./primitives/StringElement":16}],6:[function(require,module,exports){
'use strict';

var _typeof = typeof Symbol === "function" && typeof Symbol.iterator === "symbol" ? function (obj) { return typeof obj; } : function (obj) { return obj && typeof Symbol === "function" && obj.constructor === Symbol && obj !== Symbol.prototype ? "symbol" : typeof obj; };

var _createClass = function () {
  function defineProperties(target, props) {
    for (var i = 0; i < props.length; i++) {
      var descriptor = props[i];descriptor.enumerable = descriptor.enumerable || false;descriptor.configurable = true;if ("value" in descriptor) descriptor.writable = true;Object.defineProperty(target, descriptor.key, descriptor);
    }
  }return function (Constructor, protoProps, staticProps) {
    if (protoProps) defineProperties(Constructor.prototype, protoProps);if (staticProps) defineProperties(Constructor, staticProps);return Constructor;
  };
}();

function _classCallCheck(instance, Constructor) {
  if (!(instance instanceof Constructor)) {
    throw new TypeError("Cannot call a class as a function");
  }
}

function _possibleConstructorReturn(self, call) {
  if (!self) {
    throw new ReferenceError("this hasn't been initialised - super() hasn't been called");
  }return call && ((typeof call === "undefined" ? "undefined" : _typeof(call)) === "object" || typeof call === "function") ? call : self;
}

function _inherits(subClass, superClass) {
  if (typeof superClass !== "function" && superClass !== null) {
    throw new TypeError("Super expression must either be null or a function, not " + (typeof superClass === "undefined" ? "undefined" : _typeof(superClass)));
  }subClass.prototype = Object.create(superClass && superClass.prototype, { constructor: { value: subClass, enumerable: false, writable: true, configurable: true } });if (superClass) Object.setPrototypeOf ? Object.setPrototypeOf(subClass, superClass) : subClass.__proto__ = superClass;
}

var Element = require('../primitives/Element');

/** Hyperlinking MAY be used to link to other resources, provide links to
 * instructions on how to process a given element (by way of a profile or
 * other means), and may be used to provide meta data about the element in
 * which it's found. The meaning and purpose of the hyperlink is defined by
 * the link relation according to RFC 5988.
 *
 * @class LinkElement
 *
 * @param content
 * @param meta
 * @param attributes
 */
module.exports = function (_Element) {
  _inherits(LinkElement, _Element);

  function LinkElement(content, meta, attributes) {
    _classCallCheck(this, LinkElement);

    var _this = _possibleConstructorReturn(this, (LinkElement.__proto__ || Object.getPrototypeOf(LinkElement)).call(this, content || [], meta, attributes));

    _this.element = 'link';
    return _this;
  }

  /**
   * The relation identifier for the link, as defined in RFC 5988.
   * @type StringElement
   */

  _createClass(LinkElement, [{
    key: 'relation',
    get: function get() {
      return this.attributes.get('relation');
    },
    set: function set(relation) {
      this.attributes.set('relation', relation);
    }

    /**
     * The URI for the given link.
     * @type StringElement
     */

  }, {
    key: 'href',
    get: function get() {
      return this.attributes.get('href');
    },
    set: function set(href) {
      this.attributes.set('href', href);
    }
  }]);

  return LinkElement;
}(Element);

},{"../primitives/Element":11}],7:[function(require,module,exports){
'use strict';

var _typeof = typeof Symbol === "function" && typeof Symbol.iterator === "symbol" ? function (obj) { return typeof obj; } : function (obj) { return obj && typeof Symbol === "function" && obj.constructor === Symbol && obj !== Symbol.prototype ? "symbol" : typeof obj; };

var _createClass = function () {
  function defineProperties(target, props) {
    for (var i = 0; i < props.length; i++) {
      var descriptor = props[i];descriptor.enumerable = descriptor.enumerable || false;descriptor.configurable = true;if ("value" in descriptor) descriptor.writable = true;Object.defineProperty(target, descriptor.key, descriptor);
    }
  }return function (Constructor, protoProps, staticProps) {
    if (protoProps) defineProperties(Constructor.prototype, protoProps);if (staticProps) defineProperties(Constructor, staticProps);return Constructor;
  };
}();

function _classCallCheck(instance, Constructor) {
  if (!(instance instanceof Constructor)) {
    throw new TypeError("Cannot call a class as a function");
  }
}

function _possibleConstructorReturn(self, call) {
  if (!self) {
    throw new ReferenceError("this hasn't been initialised - super() hasn't been called");
  }return call && ((typeof call === "undefined" ? "undefined" : _typeof(call)) === "object" || typeof call === "function") ? call : self;
}

function _inherits(subClass, superClass) {
  if (typeof superClass !== "function" && superClass !== null) {
    throw new TypeError("Super expression must either be null or a function, not " + (typeof superClass === "undefined" ? "undefined" : _typeof(superClass)));
  }subClass.prototype = Object.create(superClass && superClass.prototype, { constructor: { value: subClass, enumerable: false, writable: true, configurable: true } });if (superClass) Object.setPrototypeOf ? Object.setPrototypeOf(subClass, superClass) : subClass.__proto__ = superClass;
}

var Element = require('../primitives/Element');

/**
 * @class RefElement
 *
 * @param content
 * @param meta
 * @param attributes
 *
 * @extends Element
 */
module.exports = function (_Element) {
  _inherits(RefElement, _Element);

  function RefElement(content, meta, attributes) {
    _classCallCheck(this, RefElement);

    var _this = _possibleConstructorReturn(this, (RefElement.__proto__ || Object.getPrototypeOf(RefElement)).call(this, content || [], meta, attributes));

    _this.element = 'ref';

    if (!_this.path) {
      _this.path = 'element';
    }
    return _this;
  }

  /**
   * Path of referenced element to transclude instead of element itself.
   * @type StringElement
   * @default element
   */

  _createClass(RefElement, [{
    key: 'path',
    get: function get() {
      return this.attributes.get('path');
    },
    set: function set(newValue) {
      this.attributes.set('path', newValue);
    }
  }]);

  return RefElement;
}(Element);

},{"../primitives/Element":11}],8:[function(require,module,exports){
'use strict';

var Namespace = require('./Namespace');
var elements = require('./elements');

// Direct access to the Namespace class
exports.Namespace = Namespace;

// Special constructor for the Namespace class
exports.namespace = function namespace(options) {
  return new Namespace(options);
};

exports.KeyValuePair = require('./KeyValuePair');

exports.ArraySlice = elements.ArraySlice;
exports.ObjectSlice = elements.ObjectSlice;

exports.Element = elements.Element;
exports.StringElement = elements.StringElement;
exports.NumberElement = elements.NumberElement;
exports.BooleanElement = elements.BooleanElement;
exports.NullElement = elements.NullElement;
exports.ArrayElement = elements.ArrayElement;
exports.ObjectElement = elements.ObjectElement;
exports.MemberElement = elements.MemberElement;
exports.RefElement = elements.RefElement;
exports.LinkElement = elements.LinkElement;

exports.refract = elements.refract;

exports.JSONSerialiser = require('./serialisers/JSONSerialiser');
exports.JSON06Serialiser = require('./serialisers/JSON06Serialiser');

},{"./KeyValuePair":2,"./Namespace":3,"./elements":5,"./serialisers/JSON06Serialiser":17,"./serialisers/JSONSerialiser":18}],9:[function(require,module,exports){
'use strict';

var _typeof = typeof Symbol === "function" && typeof Symbol.iterator === "symbol" ? function (obj) { return typeof obj; } : function (obj) { return obj && typeof Symbol === "function" && obj.constructor === Symbol && obj !== Symbol.prototype ? "symbol" : typeof obj; };

var _createClass = function () {
  function defineProperties(target, props) {
    for (var i = 0; i < props.length; i++) {
      var descriptor = props[i];descriptor.enumerable = descriptor.enumerable || false;descriptor.configurable = true;if ("value" in descriptor) descriptor.writable = true;Object.defineProperty(target, descriptor.key, descriptor);
    }
  }return function (Constructor, protoProps, staticProps) {
    if (protoProps) defineProperties(Constructor.prototype, protoProps);if (staticProps) defineProperties(Constructor, staticProps);return Constructor;
  };
}();

function _classCallCheck(instance, Constructor) {
  if (!(instance instanceof Constructor)) {
    throw new TypeError("Cannot call a class as a function");
  }
}

function _possibleConstructorReturn(self, call) {
  if (!self) {
    throw new ReferenceError("this hasn't been initialised - super() hasn't been called");
  }return call && ((typeof call === "undefined" ? "undefined" : _typeof(call)) === "object" || typeof call === "function") ? call : self;
}

function _inherits(subClass, superClass) {
  if (typeof superClass !== "function" && superClass !== null) {
    throw new TypeError("Super expression must either be null or a function, not " + (typeof superClass === "undefined" ? "undefined" : _typeof(superClass)));
  }subClass.prototype = Object.create(superClass && superClass.prototype, { constructor: { value: subClass, enumerable: false, writable: true, configurable: true } });if (superClass) Object.setPrototypeOf ? Object.setPrototypeOf(subClass, superClass) : subClass.__proto__ = superClass;
}

var negate = require('lodash/negate');
var Element = require('./Element');
var ArraySlice = require('../ArraySlice');

/**
 * @class
 *
 * @param {Element[]} content
 * @param meta
 * @param attributes
 */

var ArrayElement = function (_Element) {
  _inherits(ArrayElement, _Element);

  function ArrayElement(content, meta, attributes) {
    _classCallCheck(this, ArrayElement);

    var _this = _possibleConstructorReturn(this, (ArrayElement.__proto__ || Object.getPrototypeOf(ArrayElement)).call(this, content || [], meta, attributes));

    _this.element = 'array';
    return _this;
  }

  _createClass(ArrayElement, [{
    key: 'primitive',
    value: function primitive() {
      return 'array';
    }

    /**
     * @returns {Element}
     */

  }, {
    key: 'get',
    value: function get(index) {
      return this.content[index];
    }

    /**
     * Helper for returning the value of an item
     * This works for both ArrayElement and ObjectElement instances
     */

  }, {
    key: 'getValue',
    value: function getValue(indexOrKey) {
      var item = this.get(indexOrKey);

      if (item) {
        return item.toValue();
      }

      return undefined;
    }

    /**
     * @returns {Element}
     */

  }, {
    key: 'getIndex',
    value: function getIndex(index) {
      return this.content[index];
    }
  }, {
    key: 'set',
    value: function set(index, value) {
      this.content[index] = this.refract(value);
      return this;
    }
  }, {
    key: 'remove',
    value: function remove(index) {
      var removed = this.content.splice(index, 1);

      if (removed.length) {
        return removed[0];
      }

      return null;
    }

    /**
     * @param callback - Function to execute for each element
     * @param thisArg - Value to use as this (i.e the reference Object) when executing callback
     */

  }, {
    key: 'map',
    value: function map(callback, thisArg) {
      return this.content.map(callback, thisArg);
    }

    /**
     * Maps and then flattens the results.
     * @param callback - Function to execute for each element.
     * @param thisArg - Value to use as this (i.e the reference Object) when executing callback
     * @returns {array}
     */

  }, {
    key: 'flatMap',
    value: function flatMap(callback, thisArg) {
      return this.map(callback, thisArg).reduce(function (a, b) {
        return a.concat(b);
      }, []);
    }

    /**
     * Returns an array containing the truthy results of calling the given transformation with each element of this sequence
     * @param transform - A closure that accepts an element of this array as its argument and returns an optional value.
     * @param thisArg - Value to use as this (i.e the reference Object) when executing callback
     * @memberof ArrayElement.prototype
     * @returns An array of the non-undefined results of calling transform with each element of the array
     */

  }, {
    key: 'compactMap',
    value: function compactMap(transform, thisArg) {
      var results = [];

      this.forEach(function (element) {
        var result = transform.bind(thisArg)(element);

        if (result) {
          results.push(result);
        }
      });

      return results;
    }

    /**
     * @param callback - Function to execute for each element
     * @param thisArg - Value to use as this (i.e the reference Object) when executing callback
     * @returns {ArraySlice}
     */

  }, {
    key: 'filter',
    value: function filter(callback, thisArg) {
      return new ArraySlice(this.content.filter(callback, thisArg));
    }

    /**
     * @param callback - Function to execute for each element
     * @param thisArg - Value to use as this (i.e the reference Object) when executing callback
     * @returns {ArraySlice}
     */

  }, {
    key: 'reject',
    value: function reject(callback, thisArg) {
      return this.filter(negate(callback), thisArg);
    }

    /**
     * This is a reduce function specifically for Minim arrays and objects. It
     * allows for returning normal values or Minim instances, so it converts any
     * primitives on each step.
     */

  }, {
    key: 'reduce',
    value: function reduce(callback, initialValue) {
      var startIndex = void 0;
      var memo = void 0;

      // Allows for defining a starting value of the reduce
      if (initialValue !== undefined) {
        startIndex = 0;
        memo = this.refract(initialValue);
      } else {
        startIndex = 1;
        // Object Element content items are member elements. Because of this,
        // the memo should start out as the member value rather than the
        // actual member itself.
        memo = this.primitive() === 'object' ? this.first.value : this.first;
      }

      // Sending each function call to the registry allows for passing Minim
      // instances through the function return. This means you can return
      // primitive values or return Minim instances and reduce will still work.
      for (var i = startIndex; i < this.length; i += 1) {
        var item = this.content[i];

        if (this.primitive() === 'object') {
          memo = this.refract(callback(memo, item.value, item.key, item, this));
        } else {
          memo = this.refract(callback(memo, item, i, this));
        }
      }

      return memo;
    }

    /**
     * @callback forEachCallback
     * @param {Element} currentValue
     * @param {NumberElement} index
     */

    /**
     * @param {forEachCallback} callback - Function to execute for each element
     * @param thisArg - Value to use as this (i.e the reference Object) when executing callback
     * @memberof ArrayElement.prototype
     */

  }, {
    key: 'forEach',
    value: function forEach(callback, thisArg) {
      var _this2 = this;

      this.content.forEach(function (item, index) {
        callback.bind(thisArg)(item, _this2.refract(index));
      });
    }

    /**
     * @returns {Element}
     */

  }, {
    key: 'shift',
    value: function shift() {
      return this.content.shift();
    }

    /**
     * @param value
     */

  }, {
    key: 'unshift',
    value: function unshift(value) {
      this.content.unshift(this.refract(value));
    }

    /**
     * @param value
     */

  }, {
    key: 'push',
    value: function push(value) {
      this.content.push(this.refract(value));
      return this;
    }

    /**
     * @param value
     */

  }, {
    key: 'add',
    value: function add(value) {
      this.push(value);
    }

    /**
     * Recusively search all descendents using a condition function.
     * @returns {Element[]}
     */

  }, {
    key: 'findElements',
    value: function findElements(condition, givenOptions) {
      var options = givenOptions || {};
      var recursive = !!options.recursive;
      var results = options.results === undefined ? [] : options.results;

      // The forEach method for Object Elements returns value, key, and member.
      // This passes those along to the condition function below.
      this.forEach(function (item, keyOrIndex, member) {
        // We use duck-typing here to support any registered class that
        // may contain other elements.
        if (recursive && item.findElements !== undefined) {
          item.findElements(condition, {
            results: results,
            recursive: recursive
          });
        }

        if (condition(item, keyOrIndex, member)) {
          results.push(item);
        }
      });

      return results;
    }

    /**
     * Recusively search all descendents using a condition function.
     * @param condition
     * @returns {ArraySlice}
     */

  }, {
    key: 'find',
    value: function find(condition) {
      return new ArraySlice(this.findElements(condition, { recursive: true }));
    }

    /**
     * @param {string} element
     * @returns {ArraySlice}
     */

  }, {
    key: 'findByElement',
    value: function findByElement(element) {
      return this.find(function (item) {
        return item.element === element;
      });
    }

    /**
     * @param {string} className
     * @returns {ArraySlice}
     * @memberof ArrayElement.prototype
     */

  }, {
    key: 'findByClass',
    value: function findByClass(className) {
      return this.find(function (item) {
        return item.classes.includes(className);
      });
    }

    /**
     * Search the tree recursively and find the element with the matching ID
     * @param {string} id
     * @returns {Element}
     * @memberof ArrayElement.prototype
     */

  }, {
    key: 'getById',
    value: function getById(id) {
      return this.find(function (item) {
        return item.id.toValue() === id;
      }).first;
    }

    /**
     * Looks for matching children using deep equality
     * @param value
     * @returns {boolean}
     */

  }, {
    key: 'includes',
    value: function includes(value) {
      return this.content.some(function (element) {
        return element.equals(value);
      });
    }

    /**
     * Looks for matching children using deep equality
     * @param value
     * @returns {boolean}
     * @see includes
     * @deprecated method was replaced by includes
     */

  }, {
    key: 'contains',
    value: function contains(value) {
      return this.includes(value);
    }

    // Fantasy Land

    /**
     * @returns {ArrayElement} An empty array element
     */

  }, {
    key: 'empty',
    value: function empty() {
      return new this.constructor([]);
    }
  }, {
    key: 'fantasy-land/empty',
    value: function fantasyLandEmpty() {
      return this.empty();
    }

    /**
     * @param {ArrayElement} other
     * @returns {ArrayElement}
     */

  }, {
    key: 'concat',
    value: function concat(other) {
      return new this.constructor(this.content.concat(other.content));
    }
  }, {
    key: 'fantasy-land/concat',
    value: function fantasyLandConcat(other) {
      return this.concat(other);
    }
  }, {
    key: 'fantasy-land/map',
    value: function fantasyLandMap(transform) {
      return new this.constructor(this.map(transform));
    }
  }, {
    key: 'fantasy-land/chain',
    value: function fantasyLandChain(transform) {
      return this.map(function (element) {
        return transform(element);
      }, this).reduce(function (a, b) {
        return a.concat(b);
      }, this.empty());
    }
  }, {
    key: 'fantasy-land/filter',
    value: function fantasyLandFilter(callback) {
      return new this.constructor(this.content.filter(callback));
    }
  }, {
    key: 'fantasy-land/reduce',
    value: function fantasyLandReduce(transform, initialValue) {
      return this.content.reduce(transform, initialValue);
    }

    /**
     * Returns the length of the collection
     * @type number
     */

  }, {
    key: 'length',
    get: function get() {
      return this.content.length;
    }

    /**
     * Returns whether the collection is empty
     * @type boolean
     */

  }, {
    key: 'isEmpty',
    get: function get() {
      return this.content.length === 0;
    }

    /**
     * Return the first item in the collection
     * @type Element
     */

  }, {
    key: 'first',
    get: function get() {
      return this.getIndex(0);
    }

    /**
     * Return the second item in the collection
     * @type Element
     */

  }, {
    key: 'second',
    get: function get() {
      return this.getIndex(1);
    }

    /**
     * Return the last item in the collection
     * @type Element
     */

  }, {
    key: 'last',
    get: function get() {
      return this.getIndex(this.length - 1);
    }
  }]);

  return ArrayElement;
}(Element);

/**
 * @returns {ArrayElement} An empty array element
 */

ArrayElement.empty = function empty() {
  return new this();
};

ArrayElement['fantasy-land/empty'] = ArrayElement.empty;

if (typeof Symbol !== 'undefined') {
  ArrayElement.prototype[Symbol.iterator] = function symbol() {
    return this.content[Symbol.iterator]();
  };
}

module.exports = ArrayElement;

},{"../ArraySlice":1,"./Element":11,"lodash/negate":110}],10:[function(require,module,exports){
'use strict';

var _typeof = typeof Symbol === "function" && typeof Symbol.iterator === "symbol" ? function (obj) { return typeof obj; } : function (obj) { return obj && typeof Symbol === "function" && obj.constructor === Symbol && obj !== Symbol.prototype ? "symbol" : typeof obj; };

var _createClass = function () {
  function defineProperties(target, props) {
    for (var i = 0; i < props.length; i++) {
      var descriptor = props[i];descriptor.enumerable = descriptor.enumerable || false;descriptor.configurable = true;if ("value" in descriptor) descriptor.writable = true;Object.defineProperty(target, descriptor.key, descriptor);
    }
  }return function (Constructor, protoProps, staticProps) {
    if (protoProps) defineProperties(Constructor.prototype, protoProps);if (staticProps) defineProperties(Constructor, staticProps);return Constructor;
  };
}();

function _classCallCheck(instance, Constructor) {
  if (!(instance instanceof Constructor)) {
    throw new TypeError("Cannot call a class as a function");
  }
}

function _possibleConstructorReturn(self, call) {
  if (!self) {
    throw new ReferenceError("this hasn't been initialised - super() hasn't been called");
  }return call && ((typeof call === "undefined" ? "undefined" : _typeof(call)) === "object" || typeof call === "function") ? call : self;
}

function _inherits(subClass, superClass) {
  if (typeof superClass !== "function" && superClass !== null) {
    throw new TypeError("Super expression must either be null or a function, not " + (typeof superClass === "undefined" ? "undefined" : _typeof(superClass)));
  }subClass.prototype = Object.create(superClass && superClass.prototype, { constructor: { value: subClass, enumerable: false, writable: true, configurable: true } });if (superClass) Object.setPrototypeOf ? Object.setPrototypeOf(subClass, superClass) : subClass.__proto__ = superClass;
}

var Element = require('./Element');

/**
 * @class BooleanElement
 *
 * @param {boolean} content
 * @param meta
 * @param attributes
 */
module.exports = function (_Element) {
  _inherits(BooleanElement, _Element);

  function BooleanElement(content, meta, attributes) {
    _classCallCheck(this, BooleanElement);

    var _this = _possibleConstructorReturn(this, (BooleanElement.__proto__ || Object.getPrototypeOf(BooleanElement)).call(this, content, meta, attributes));

    _this.element = 'boolean';
    return _this;
  }

  _createClass(BooleanElement, [{
    key: 'primitive',
    value: function primitive() {
      return 'boolean';
    }
  }]);

  return BooleanElement;
}(Element);

},{"./Element":11}],11:[function(require,module,exports){
'use strict';

var _typeof2 = typeof Symbol === "function" && typeof Symbol.iterator === "symbol" ? function (obj) { return typeof obj; } : function (obj) { return obj && typeof Symbol === "function" && obj.constructor === Symbol && obj !== Symbol.prototype ? "symbol" : typeof obj; };

var _typeof = typeof Symbol === "function" && _typeof2(Symbol.iterator) === "symbol" ? function (obj) {
  return typeof obj === "undefined" ? "undefined" : _typeof2(obj);
} : function (obj) {
  return obj && typeof Symbol === "function" && obj.constructor === Symbol && obj !== Symbol.prototype ? "symbol" : typeof obj === "undefined" ? "undefined" : _typeof2(obj);
};

var _createClass = function () {
  function defineProperties(target, props) {
    for (var i = 0; i < props.length; i++) {
      var descriptor = props[i];descriptor.enumerable = descriptor.enumerable || false;descriptor.configurable = true;if ("value" in descriptor) descriptor.writable = true;Object.defineProperty(target, descriptor.key, descriptor);
    }
  }return function (Constructor, protoProps, staticProps) {
    if (protoProps) defineProperties(Constructor.prototype, protoProps);if (staticProps) defineProperties(Constructor, staticProps);return Constructor;
  };
}();

function _classCallCheck(instance, Constructor) {
  if (!(instance instanceof Constructor)) {
    throw new TypeError("Cannot call a class as a function");
  }
}

var isEqual = require('lodash/isEqual');
var KeyValuePair = require('../KeyValuePair');
var ArraySlice = require('../ArraySlice.js');

/**
 * @class
 *
 * @param content
 * @param meta
 * @param attributes
 *
 * @property {string} element
 */

var Element = function () {
  function Element(content, meta, attributes) {
    _classCallCheck(this, Element);

    // Lazy load this.meta and this.attributes because it's a Minim element
    // Otherwise, we get into circuluar calls
    if (meta) {
      this.meta = meta;
    }

    if (attributes) {
      this.attributes = attributes;
    }

    this.content = content;
  }

  /**
   * Freezes the element to prevent any mutation.
   * A frozen element will add `parent` property to every child element
   * to allow traversing up the element tree.
   */

  _createClass(Element, [{
    key: 'freeze',
    value: function freeze() {
      var _this = this;

      if (Object.isFrozen(this)) {
        return;
      }

      if (this._meta) {
        this.meta.parent = this;
        this.meta.freeze();
      }

      if (this._attributes) {
        this.attributes.parent = this;
        this.attributes.freeze();
      }

      this.children.forEach(function (element) {
        element.parent = _this;
        element.freeze();
      }, this);

      if (this.content && Array.isArray(this.content)) {
        Object.freeze(this.content);
      }

      Object.freeze(this);
    }
  }, {
    key: 'primitive',
    value: function primitive() {}

    /**
     * Creates a deep clone of the instance
     */

  }, {
    key: 'clone',
    value: function clone() {
      var copy = new this.constructor();

      copy.element = this.element;

      if (this.meta.length) {
        copy._meta = this.meta.clone();
      }

      if (this.attributes.length) {
        copy._attributes = this.attributes.clone();
      }

      if (this.content) {
        if (this.content.clone) {
          copy.content = this.content.clone();
        } else if (Array.isArray(this.content)) {
          copy.content = this.content.map(function (element) {
            return element.clone();
          });
        } else {
          copy.content = this.content;
        }
      } else {
        copy.content = this.content;
      }

      return copy;
    }

    /**
     */

  }, {
    key: 'toValue',
    value: function toValue() {
      if (this.content instanceof Element) {
        return this.content.toValue();
      }

      if (this.content instanceof KeyValuePair) {
        return {
          key: this.content.key.toValue(),
          value: this.content.value ? this.content.value.toValue() : undefined
        };
      }

      if (this.content && this.content.map) {
        return this.content.map(function (element) {
          return element.toValue();
        }, this);
      }

      return this.content;
    }

    /**
     * Creates a reference pointing at the Element
     * @returns {RefElement}
     * @memberof Element.prototype
     */

  }, {
    key: 'toRef',
    value: function toRef(path) {
      if (this.id.toValue() === '') {
        throw Error('Cannot create reference to an element that does not contain an ID');
      }

      var ref = new this.RefElement(this.id.toValue());

      if (path) {
        ref.path = path;
      }

      return ref;
    }

    /**
     * Finds the given elements in the element tree.
     * When providing multiple element names, you must first freeze the element.
     *
     * @param names {...elementNames}
     * @returns {ArraySlice}
     */

  }, {
    key: 'findRecursive',
    value: function findRecursive() {
      for (var _len = arguments.length, elementNames = Array(_len), _key = 0; _key < _len; _key++) {
        elementNames[_key] = arguments[_key];
      }

      if (arguments.length > 1 && !this.isFrozen) {
        throw new Error('Cannot find recursive with multiple element names without first freezing the element. Call `element.freeze()`');
      }

      var elementName = elementNames.pop();
      var elements = new ArraySlice();

      var append = function append(array, element) {
        array.push(element);
        return array;
      };

      // Checks the given element and appends element/sub-elements
      // that match element name to given array
      var checkElement = function checkElement(array, element) {
        if (element.element === elementName) {
          array.push(element);
        }

        var items = element.findRecursive(elementName);
        if (items) {
          items.reduce(append, array);
        }

        if (element.content instanceof KeyValuePair) {
          if (element.content.key) {
            checkElement(array, element.content.key);
          }

          if (element.content.value) {
            checkElement(array, element.content.value);
          }
        }

        return array;
      };

      if (this.content) {
        // Direct Element
        if (this.content.element) {
          checkElement(elements, this.content);
        }

        // Element Array
        if (Array.isArray(this.content)) {
          this.content.reduce(checkElement, elements);
        }
      }

      if (!elementNames.isEmpty) {
        elements = elements.filter(function (element) {
          var parentElements = element.parents.map(function (e) {
            return e.element;
          });

          // eslint-disable-next-line no-restricted-syntax
          for (var namesIndex in elementNames) {
            var name = elementNames[namesIndex];
            var index = parentElements.indexOf(name);

            if (index !== -1) {
              parentElements = parentElements.splice(0, index);
            } else {
              return false;
            }
          }

          return true;
        });
      }

      return elements;
    }
  }, {
    key: 'set',
    value: function set(content) {
      this.content = content;
      return this;
    }
  }, {
    key: 'equals',
    value: function equals(value) {
      return isEqual(this.toValue(), value);
    }
  }, {
    key: 'getMetaProperty',
    value: function getMetaProperty(name, value) {
      if (!this.meta.hasKey(name)) {
        if (this.isFrozen) {
          var element = this.refract(value);
          element.freeze();
          return element;
        }

        this.meta.set(name, value);
      }

      return this.meta.get(name);
    }
  }, {
    key: 'setMetaProperty',
    value: function setMetaProperty(name, value) {
      this.meta.set(name, value);
    }

    /**
     * @type String
     */

  }, {
    key: 'element',
    get: function get() {
      // Returns 'element' so we don't have undefined as element
      return this._storedElement || 'element';
    },
    set: function set(element) {
      this._storedElement = element;
    }
  }, {
    key: 'content',
    get: function get() {
      return this._content;
    },
    set: function set(value) {
      var _this2 = this;

      if (value instanceof Element) {
        this._content = value;
      } else if (value instanceof ArraySlice) {
        this.content = value.elements;
      } else if (typeof value == 'string' || typeof value == 'number' || typeof value == 'boolean' || value === 'null' || value == undefined) {
        // Primitive Values
        this._content = value;
      } else if (value instanceof KeyValuePair) {
        this._content = value;
      } else if (Array.isArray(value)) {
        this._content = value.map(this.refract);
      } else if ((typeof value === 'undefined' ? 'undefined' : _typeof(value)) === 'object') {
        this._content = Object.keys(value).map(function (key) {
          return new _this2.MemberElement(key, value[key]);
        });
      } else {
        throw new Error('Cannot set content to given value');
      }
    }

    /**
     * @type ObjectElement
     */

  }, {
    key: 'meta',
    get: function get() {
      if (!this._meta) {
        if (this.isFrozen) {
          var meta = new this.ObjectElement();
          meta.freeze();
          return meta;
        }

        this._meta = new this.ObjectElement();
      }

      return this._meta;
    },
    set: function set(value) {
      if (value instanceof this.ObjectElement) {
        this._meta = value;
      } else {
        this.meta.set(value || {});
      }
    }

    /**
     * The attributes property defines attributes about the given instance
     * of the element, as specified by the element property.
     *
     * @type ObjectElement
     */

  }, {
    key: 'attributes',
    get: function get() {
      if (!this._attributes) {
        if (this.isFrozen) {
          var meta = new this.ObjectElement();
          meta.freeze();
          return meta;
        }

        this._attributes = new this.ObjectElement();
      }

      return this._attributes;
    },
    set: function set(value) {
      if (value instanceof this.ObjectElement) {
        this._attributes = value;
      } else {
        this.attributes.set(value || {});
      }
    }

    /**
     * Unique Identifier, MUST be unique throughout an entire element tree.
     * @type StringElement
     */

  }, {
    key: 'id',
    get: function get() {
      return this.getMetaProperty('id', '');
    },
    set: function set(element) {
      this.setMetaProperty('id', element);
    }

    /**
     * @type ArrayElement
     */

  }, {
    key: 'classes',
    get: function get() {
      return this.getMetaProperty('classes', []);
    },
    set: function set(element) {
      this.setMetaProperty('classes', element);
    }

    /**
     * Human-readable title of element
     * @type StringElement
     */

  }, {
    key: 'title',
    get: function get() {
      return this.getMetaProperty('title', '');
    },
    set: function set(element) {
      this.setMetaProperty('title', element);
    }

    /**
     * Human-readable description of element
     * @type StringElement
     */

  }, {
    key: 'description',
    get: function get() {
      return this.getMetaProperty('description', '');
    },
    set: function set(element) {
      this.setMetaProperty('description', element);
    }

    /**
     * @type ArrayElement
     */

  }, {
    key: 'links',
    get: function get() {
      return this.getMetaProperty('links', []);
    },
    set: function set(element) {
      this.setMetaProperty('links', element);
    }

    /**
     * Returns whether the element is frozen.
     * @type boolean
     * @see freeze
     */

  }, {
    key: 'isFrozen',
    get: function get() {
      return Object.isFrozen(this);
    }

    /**
     * Returns all of the parent elements.
     * @type ArraySlice
     */

  }, {
    key: 'parents',
    get: function get() {
      var parent = this.parent;

      var parents = new ArraySlice();

      while (parent) {
        parents.push(parent);

        // eslint-disable-next-line prefer-destructuring
        parent = parent.parent;
      }

      return parents;
    }

    /**
     * Returns all of the children elements found within the element.
     * @type ArraySlice
     * @see recursiveChildren
     */

  }, {
    key: 'children',
    get: function get() {
      if (Array.isArray(this.content)) {
        return new ArraySlice(this.content);
      }

      if (this.content instanceof KeyValuePair) {
        var children = new ArraySlice([this.content.key]);

        if (this.content.value) {
          children.push(this.content.value);
        }

        return children;
      }

      if (this.content instanceof Element) {
        return new ArraySlice([this.content]);
      }

      return new ArraySlice();
    }

    /**
    * Returns all of the children elements found within the element recursively.
    * @type ArraySlice
    * @see children
    */

  }, {
    key: 'recursiveChildren',
    get: function get() {
      var children = new ArraySlice();

      this.children.forEach(function (element) {
        children.push(element);

        element.recursiveChildren.forEach(function (child) {
          children.push(child);
        });
      });

      return children;
    }
  }]);

  return Element;
}();

module.exports = Element;

},{"../ArraySlice.js":1,"../KeyValuePair":2,"lodash/isEqual":100}],12:[function(require,module,exports){
'use strict';

var _typeof = typeof Symbol === "function" && typeof Symbol.iterator === "symbol" ? function (obj) { return typeof obj; } : function (obj) { return obj && typeof Symbol === "function" && obj.constructor === Symbol && obj !== Symbol.prototype ? "symbol" : typeof obj; };

var _createClass = function () {
  function defineProperties(target, props) {
    for (var i = 0; i < props.length; i++) {
      var descriptor = props[i];descriptor.enumerable = descriptor.enumerable || false;descriptor.configurable = true;if ("value" in descriptor) descriptor.writable = true;Object.defineProperty(target, descriptor.key, descriptor);
    }
  }return function (Constructor, protoProps, staticProps) {
    if (protoProps) defineProperties(Constructor.prototype, protoProps);if (staticProps) defineProperties(Constructor, staticProps);return Constructor;
  };
}();

function _classCallCheck(instance, Constructor) {
  if (!(instance instanceof Constructor)) {
    throw new TypeError("Cannot call a class as a function");
  }
}

function _possibleConstructorReturn(self, call) {
  if (!self) {
    throw new ReferenceError("this hasn't been initialised - super() hasn't been called");
  }return call && ((typeof call === "undefined" ? "undefined" : _typeof(call)) === "object" || typeof call === "function") ? call : self;
}

function _inherits(subClass, superClass) {
  if (typeof superClass !== "function" && superClass !== null) {
    throw new TypeError("Super expression must either be null or a function, not " + (typeof superClass === "undefined" ? "undefined" : _typeof(superClass)));
  }subClass.prototype = Object.create(superClass && superClass.prototype, { constructor: { value: subClass, enumerable: false, writable: true, configurable: true } });if (superClass) Object.setPrototypeOf ? Object.setPrototypeOf(subClass, superClass) : subClass.__proto__ = superClass;
}

var KeyValuePair = require('../KeyValuePair');
var Element = require('./Element');

/**
 * @class MemberElement
 *
 * @param {Element} key
 * @param {Element} value
 * @param meta
 * @param attributes
 */
module.exports = function (_Element) {
  _inherits(MemberElement, _Element);

  function MemberElement(key, value, meta, attributes) {
    _classCallCheck(this, MemberElement);

    var _this = _possibleConstructorReturn(this, (MemberElement.__proto__ || Object.getPrototypeOf(MemberElement)).call(this, new KeyValuePair(), meta, attributes));

    _this.element = 'member';
    _this.key = key;
    _this.value = value;
    return _this;
  }

  /**
   * @type Element
   */

  _createClass(MemberElement, [{
    key: 'key',
    get: function get() {
      return this.content.key;
    },
    set: function set(key) {
      this.content.key = this.refract(key);
    }

    /**
     * @type Element
     */

  }, {
    key: 'value',
    get: function get() {
      return this.content.value;
    },
    set: function set(value) {
      this.content.value = this.refract(value);
    }
  }]);

  return MemberElement;
}(Element);

},{"../KeyValuePair":2,"./Element":11}],13:[function(require,module,exports){
'use strict';

var _typeof = typeof Symbol === "function" && typeof Symbol.iterator === "symbol" ? function (obj) { return typeof obj; } : function (obj) { return obj && typeof Symbol === "function" && obj.constructor === Symbol && obj !== Symbol.prototype ? "symbol" : typeof obj; };

var _createClass = function () {
  function defineProperties(target, props) {
    for (var i = 0; i < props.length; i++) {
      var descriptor = props[i];descriptor.enumerable = descriptor.enumerable || false;descriptor.configurable = true;if ("value" in descriptor) descriptor.writable = true;Object.defineProperty(target, descriptor.key, descriptor);
    }
  }return function (Constructor, protoProps, staticProps) {
    if (protoProps) defineProperties(Constructor.prototype, protoProps);if (staticProps) defineProperties(Constructor, staticProps);return Constructor;
  };
}();

function _classCallCheck(instance, Constructor) {
  if (!(instance instanceof Constructor)) {
    throw new TypeError("Cannot call a class as a function");
  }
}

function _possibleConstructorReturn(self, call) {
  if (!self) {
    throw new ReferenceError("this hasn't been initialised - super() hasn't been called");
  }return call && ((typeof call === "undefined" ? "undefined" : _typeof(call)) === "object" || typeof call === "function") ? call : self;
}

function _inherits(subClass, superClass) {
  if (typeof superClass !== "function" && superClass !== null) {
    throw new TypeError("Super expression must either be null or a function, not " + (typeof superClass === "undefined" ? "undefined" : _typeof(superClass)));
  }subClass.prototype = Object.create(superClass && superClass.prototype, { constructor: { value: subClass, enumerable: false, writable: true, configurable: true } });if (superClass) Object.setPrototypeOf ? Object.setPrototypeOf(subClass, superClass) : subClass.__proto__ = superClass;
}

var Element = require('./Element');

/**
 */

var NullElement = function (_Element) {
  _inherits(NullElement, _Element);

  function NullElement(content, meta, attributes) {
    _classCallCheck(this, NullElement);

    var _this = _possibleConstructorReturn(this, (NullElement.__proto__ || Object.getPrototypeOf(NullElement)).call(this, content || null, meta, attributes));

    _this.element = 'null';
    return _this;
  }

  _createClass(NullElement, [{
    key: 'primitive',
    value: function primitive() {
      return 'null';
    }
  }, {
    key: 'set',
    value: function set() {
      return new Error('Cannot set the value of null');
    }
  }]);

  return NullElement;
}(Element);

module.exports = NullElement;

},{"./Element":11}],14:[function(require,module,exports){
'use strict';

var _typeof = typeof Symbol === "function" && typeof Symbol.iterator === "symbol" ? function (obj) { return typeof obj; } : function (obj) { return obj && typeof Symbol === "function" && obj.constructor === Symbol && obj !== Symbol.prototype ? "symbol" : typeof obj; };

var _createClass = function () {
  function defineProperties(target, props) {
    for (var i = 0; i < props.length; i++) {
      var descriptor = props[i];descriptor.enumerable = descriptor.enumerable || false;descriptor.configurable = true;if ("value" in descriptor) descriptor.writable = true;Object.defineProperty(target, descriptor.key, descriptor);
    }
  }return function (Constructor, protoProps, staticProps) {
    if (protoProps) defineProperties(Constructor.prototype, protoProps);if (staticProps) defineProperties(Constructor, staticProps);return Constructor;
  };
}();

function _classCallCheck(instance, Constructor) {
  if (!(instance instanceof Constructor)) {
    throw new TypeError("Cannot call a class as a function");
  }
}

function _possibleConstructorReturn(self, call) {
  if (!self) {
    throw new ReferenceError("this hasn't been initialised - super() hasn't been called");
  }return call && ((typeof call === "undefined" ? "undefined" : _typeof(call)) === "object" || typeof call === "function") ? call : self;
}

function _inherits(subClass, superClass) {
  if (typeof superClass !== "function" && superClass !== null) {
    throw new TypeError("Super expression must either be null or a function, not " + (typeof superClass === "undefined" ? "undefined" : _typeof(superClass)));
  }subClass.prototype = Object.create(superClass && superClass.prototype, { constructor: { value: subClass, enumerable: false, writable: true, configurable: true } });if (superClass) Object.setPrototypeOf ? Object.setPrototypeOf(subClass, superClass) : subClass.__proto__ = superClass;
}

var Element = require('./Element');

/**
 * @class NumberElement
 *
 * @param {number} content
 * @param meta
 * @param attributes
 */
module.exports = function (_Element) {
  _inherits(NumberElement, _Element);

  function NumberElement(content, meta, attributes) {
    _classCallCheck(this, NumberElement);

    var _this = _possibleConstructorReturn(this, (NumberElement.__proto__ || Object.getPrototypeOf(NumberElement)).call(this, content, meta, attributes));

    _this.element = 'number';
    return _this;
  }

  _createClass(NumberElement, [{
    key: 'primitive',
    value: function primitive() {
      return 'number';
    }
  }]);

  return NumberElement;
}(Element);

},{"./Element":11}],15:[function(require,module,exports){
'use strict';

var _typeof = typeof Symbol === "function" && typeof Symbol.iterator === "symbol" ? function (obj) { return typeof obj; } : function (obj) { return obj && typeof Symbol === "function" && obj.constructor === Symbol && obj !== Symbol.prototype ? "symbol" : typeof obj; };

var _createClass = function () {
  function defineProperties(target, props) {
    for (var i = 0; i < props.length; i++) {
      var descriptor = props[i];descriptor.enumerable = descriptor.enumerable || false;descriptor.configurable = true;if ("value" in descriptor) descriptor.writable = true;Object.defineProperty(target, descriptor.key, descriptor);
    }
  }return function (Constructor, protoProps, staticProps) {
    if (protoProps) defineProperties(Constructor.prototype, protoProps);if (staticProps) defineProperties(Constructor, staticProps);return Constructor;
  };
}();

function _classCallCheck(instance, Constructor) {
  if (!(instance instanceof Constructor)) {
    throw new TypeError("Cannot call a class as a function");
  }
}

function _possibleConstructorReturn(self, call) {
  if (!self) {
    throw new ReferenceError("this hasn't been initialised - super() hasn't been called");
  }return call && ((typeof call === "undefined" ? "undefined" : _typeof(call)) === "object" || typeof call === "function") ? call : self;
}

function _inherits(subClass, superClass) {
  if (typeof superClass !== "function" && superClass !== null) {
    throw new TypeError("Super expression must either be null or a function, not " + (typeof superClass === "undefined" ? "undefined" : _typeof(superClass)));
  }subClass.prototype = Object.create(superClass && superClass.prototype, { constructor: { value: subClass, enumerable: false, writable: true, configurable: true } });if (superClass) Object.setPrototypeOf ? Object.setPrototypeOf(subClass, superClass) : subClass.__proto__ = superClass;
}

var negate = require('lodash/negate');
var isObject = require('lodash/isObject');

var ArrayElement = require('./ArrayElement');
var MemberElement = require('./MemberElement');
var ObjectSlice = require('../ObjectSlice');

/**
 * @class
 *
 * @param content
 * @param meta
 * @param attributes
 */

var ObjectElement = function (_ArrayElement) {
  _inherits(ObjectElement, _ArrayElement);

  function ObjectElement(content, meta, attributes) {
    _classCallCheck(this, ObjectElement);

    var _this = _possibleConstructorReturn(this, (ObjectElement.__proto__ || Object.getPrototypeOf(ObjectElement)).call(this, content || [], meta, attributes));

    _this.element = 'object';
    return _this;
  }

  _createClass(ObjectElement, [{
    key: 'primitive',
    value: function primitive() {
      return 'object';
    }
  }, {
    key: 'toValue',
    value: function toValue() {
      return this.content.reduce(function (results, el) {
        results[el.key.toValue()] = el.value ? el.value.toValue() : undefined;
        return results;
      }, {});
    }

    /**
     * @param key
     * @returns {Element}
     */

  }, {
    key: 'get',
    value: function get(name) {
      var member = this.getMember(name);

      if (member) {
        return member.value;
      }

      return undefined;
    }

    /**
     * @param key
     * @returns {MemberElement}
     */

  }, {
    key: 'getMember',
    value: function getMember(name) {
      if (name === undefined) {
        return undefined;
      }

      return this.content.find(function (element) {
        return element.key.toValue() === name;
      });
    }

    /**
     * @param key
     */

  }, {
    key: 'remove',
    value: function remove(name) {
      var removed = null;

      this.content = this.content.filter(function (item) {
        if (item.key.toValue() === name) {
          removed = item;
          return false;
        }

        return true;
      });

      return removed;
    }

    /**
     * @param key
     * @returns {Element}
     */

  }, {
    key: 'getKey',
    value: function getKey(name) {
      var member = this.getMember(name);

      if (member) {
        return member.key;
      }

      return undefined;
    }

    /**
     * Set allows either a key/value pair to be given or an object
     * If an object is given, each key is set to its respective value
     */

  }, {
    key: 'set',
    value: function set(keyOrObject, value) {
      var _this2 = this;

      if (isObject(keyOrObject)) {
        Object.keys(keyOrObject).forEach(function (objectKey) {
          _this2.set(objectKey, keyOrObject[objectKey]);
        });

        return this;
      }

      // Store as key for clarity
      var key = keyOrObject;
      var member = this.getMember(key);

      if (member) {
        member.value = value;
      } else {
        this.content.push(new MemberElement(key, value));
      }

      return this;
    }

    /**
     */

  }, {
    key: 'keys',
    value: function keys() {
      return this.content.map(function (item) {
        return item.key.toValue();
      });
    }

    /**
     */

  }, {
    key: 'values',
    value: function values() {
      return this.content.map(function (item) {
        return item.value.toValue();
      });
    }

    /**
     * @returns {boolean}
     */

  }, {
    key: 'hasKey',
    value: function hasKey(value) {
      return this.content.some(function (member) {
        return member.key.equals(value);
      });
    }

    /**
     * @returns {array}
     */

  }, {
    key: 'items',
    value: function items() {
      return this.content.map(function (item) {
        return [item.key.toValue(), item.value.toValue()];
      });
    }

    /**
     * @param callback
     * @param thisArg - Value to use as this (i.e the reference Object) when executing callback
     */

  }, {
    key: 'map',
    value: function map(callback, thisArg) {
      return this.content.map(function (item) {
        return callback.bind(thisArg)(item.value, item.key, item);
      });
    }

    /**
     * Returns an array containing the truthy results of calling the given transformation with each element of this sequence
     * @param transform - A closure that accepts the value, key and member element of this object as its argument and returns an optional value.
     * @param thisArg - Value to use as this (i.e the reference Object) when executing callback
     * @returns An array of the non-undefined results of calling transform with each element of the array
     */

  }, {
    key: 'compactMap',
    value: function compactMap(callback, thisArg) {
      var results = [];

      this.forEach(function (value, key, member) {
        var result = callback.bind(thisArg)(value, key, member);

        if (result) {
          results.push(result);
        }
      });

      return results;
    }

    /**
     * @param callback
     * @param thisArg - Value to use as this (i.e the reference Object) when executing callback
     *
     * @returns {ObjectSlice}
     */

  }, {
    key: 'filter',
    value: function filter(callback, thisArg) {
      return new ObjectSlice(this.content).filter(callback, thisArg);
    }

    /**
     * @param callback
     * @param thisArg - Value to use as this (i.e the reference Object) when executing callback
     *
     * @returns {ObjectSlice}
     *
     * @memberof ObjectElement.prototype
     */

  }, {
    key: 'reject',
    value: function reject(callback, thisArg) {
      return this.filter(negate(callback), thisArg);
    }

    /**
     * @param callback
     * @param thisArg - Value to use as this (i.e the reference Object) when executing callback
     *
     * @memberof ObjectElement.prototype
     */

  }, {
    key: 'forEach',
    value: function forEach(callback, thisArg) {
      return this.content.forEach(function (item) {
        return callback.bind(thisArg)(item.value, item.key, item);
      });
    }
  }]);

  return ObjectElement;
}(ArrayElement);

module.exports = ObjectElement;

},{"../ObjectSlice":4,"./ArrayElement":9,"./MemberElement":12,"lodash/isObject":105,"lodash/negate":110}],16:[function(require,module,exports){
'use strict';

var _typeof = typeof Symbol === "function" && typeof Symbol.iterator === "symbol" ? function (obj) { return typeof obj; } : function (obj) { return obj && typeof Symbol === "function" && obj.constructor === Symbol && obj !== Symbol.prototype ? "symbol" : typeof obj; };

var _createClass = function () {
  function defineProperties(target, props) {
    for (var i = 0; i < props.length; i++) {
      var descriptor = props[i];descriptor.enumerable = descriptor.enumerable || false;descriptor.configurable = true;if ("value" in descriptor) descriptor.writable = true;Object.defineProperty(target, descriptor.key, descriptor);
    }
  }return function (Constructor, protoProps, staticProps) {
    if (protoProps) defineProperties(Constructor.prototype, protoProps);if (staticProps) defineProperties(Constructor, staticProps);return Constructor;
  };
}();

function _classCallCheck(instance, Constructor) {
  if (!(instance instanceof Constructor)) {
    throw new TypeError("Cannot call a class as a function");
  }
}

function _possibleConstructorReturn(self, call) {
  if (!self) {
    throw new ReferenceError("this hasn't been initialised - super() hasn't been called");
  }return call && ((typeof call === "undefined" ? "undefined" : _typeof(call)) === "object" || typeof call === "function") ? call : self;
}

function _inherits(subClass, superClass) {
  if (typeof superClass !== "function" && superClass !== null) {
    throw new TypeError("Super expression must either be null or a function, not " + (typeof superClass === "undefined" ? "undefined" : _typeof(superClass)));
  }subClass.prototype = Object.create(superClass && superClass.prototype, { constructor: { value: subClass, enumerable: false, writable: true, configurable: true } });if (superClass) Object.setPrototypeOf ? Object.setPrototypeOf(subClass, superClass) : subClass.__proto__ = superClass;
}

var Element = require('./Element');

/**
 * @class StringElement
 *
 * @param {string} content
 * @param meta
 * @param attributes
 */
module.exports = function (_Element) {
  _inherits(StringElement, _Element);

  function StringElement(content, meta, attributes) {
    _classCallCheck(this, StringElement);

    var _this = _possibleConstructorReturn(this, (StringElement.__proto__ || Object.getPrototypeOf(StringElement)).call(this, content, meta, attributes));

    _this.element = 'string';
    return _this;
  }

  _createClass(StringElement, [{
    key: 'primitive',
    value: function primitive() {
      return 'string';
    }

    /**
     * The length of the string.
     * @type number
     */

  }, {
    key: 'length',
    get: function get() {
      return this.content.length;
    }
  }]);

  return StringElement;
}(Element);

},{"./Element":11}],17:[function(require,module,exports){
'use strict';

var _typeof = typeof Symbol === "function" && typeof Symbol.iterator === "symbol" ? function (obj) { return typeof obj; } : function (obj) { return obj && typeof Symbol === "function" && obj.constructor === Symbol && obj !== Symbol.prototype ? "symbol" : typeof obj; };

var _slicedToArray = function () {
  function sliceIterator(arr, i) {
    var _arr = [];var _n = true;var _d = false;var _e = undefined;try {
      for (var _i = arr[Symbol.iterator](), _s; !(_n = (_s = _i.next()).done); _n = true) {
        _arr.push(_s.value);if (i && _arr.length === i) break;
      }
    } catch (err) {
      _d = true;_e = err;
    } finally {
      try {
        if (!_n && _i["return"]) _i["return"]();
      } finally {
        if (_d) throw _e;
      }
    }return _arr;
  }return function (arr, i) {
    if (Array.isArray(arr)) {
      return arr;
    } else if (Symbol.iterator in Object(arr)) {
      return sliceIterator(arr, i);
    } else {
      throw new TypeError("Invalid attempt to destructure non-iterable instance");
    }
  };
}();

var _createClass = function () {
  function defineProperties(target, props) {
    for (var i = 0; i < props.length; i++) {
      var descriptor = props[i];descriptor.enumerable = descriptor.enumerable || false;descriptor.configurable = true;if ("value" in descriptor) descriptor.writable = true;Object.defineProperty(target, descriptor.key, descriptor);
    }
  }return function (Constructor, protoProps, staticProps) {
    if (protoProps) defineProperties(Constructor.prototype, protoProps);if (staticProps) defineProperties(Constructor, staticProps);return Constructor;
  };
}();

function _classCallCheck(instance, Constructor) {
  if (!(instance instanceof Constructor)) {
    throw new TypeError("Cannot call a class as a function");
  }
}

function _possibleConstructorReturn(self, call) {
  if (!self) {
    throw new ReferenceError("this hasn't been initialised - super() hasn't been called");
  }return call && ((typeof call === "undefined" ? "undefined" : _typeof(call)) === "object" || typeof call === "function") ? call : self;
}

function _inherits(subClass, superClass) {
  if (typeof superClass !== "function" && superClass !== null) {
    throw new TypeError("Super expression must either be null or a function, not " + (typeof superClass === "undefined" ? "undefined" : _typeof(superClass)));
  }subClass.prototype = Object.create(superClass && superClass.prototype, { constructor: { value: subClass, enumerable: false, writable: true, configurable: true } });if (superClass) Object.setPrototypeOf ? Object.setPrototypeOf(subClass, superClass) : subClass.__proto__ = superClass;
}

var JSONSerialiser = require('./JSONSerialiser');

module.exports = function (_JSONSerialiser) {
  _inherits(JSON06Serialiser, _JSONSerialiser);

  function JSON06Serialiser() {
    _classCallCheck(this, JSON06Serialiser);

    return _possibleConstructorReturn(this, (JSON06Serialiser.__proto__ || Object.getPrototypeOf(JSON06Serialiser)).apply(this, arguments));
  }

  _createClass(JSON06Serialiser, [{
    key: 'serialise',
    value: function serialise(element) {
      if (!(element instanceof this.namespace.elements.Element)) {
        throw new TypeError('Given element `' + element + '` is not an Element instance');
      }

      var variable = void 0;
      if (element._attributes && element.attributes.get('variable')) {
        variable = element.attributes.get('variable');
      }

      var payload = {
        element: element.element
      };

      if (element._meta && element._meta.length > 0) {
        payload.meta = this.serialiseObject(element.meta);
      }

      var isEnum = element.element === 'enum' || element.attributes.keys().indexOf('enumerations') !== -1;

      if (isEnum) {
        var attributes = this.enumSerialiseAttributes(element);

        if (attributes) {
          payload.attributes = attributes;
        }
      } else if (element._attributes && element._attributes.length > 0) {
        var _attributes = element.attributes;

        // Meta attribute was renamed to metadata

        if (_attributes.get('metadata')) {
          _attributes = _attributes.clone();
          _attributes.set('meta', _attributes.get('metadata'));
          _attributes.remove('metadata');
        }

        if (element.element === 'member' && variable) {
          _attributes = _attributes.clone();
          _attributes.remove('variable');
        }

        if (_attributes.length > 0) {
          payload.attributes = this.serialiseObject(_attributes);
        }
      }

      if (isEnum) {
        payload.content = this.enumSerialiseContent(element, payload);
      } else if (this[element.element + 'SerialiseContent']) {
        payload.content = this[element.element + 'SerialiseContent'](element, payload);
      } else if (element.content !== undefined) {
        var content = void 0;

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
  }, {
    key: 'shouldSerialiseContent',
    value: function shouldSerialiseContent(element, content) {
      if (element.element === 'parseResult' || element.element === 'httpRequest' || element.element === 'httpResponse' || element.element === 'category' || element.element === 'link') {
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
  }, {
    key: 'refSerialiseContent',
    value: function refSerialiseContent(element, payload) {
      delete payload.attributes;

      return {
        href: element.toValue(),
        path: element.path.toValue()
      };
    }
  }, {
    key: 'sourceMapSerialiseContent',
    value: function sourceMapSerialiseContent(element) {
      return element.toValue();
    }
  }, {
    key: 'dataStructureSerialiseContent',
    value: function dataStructureSerialiseContent(element) {
      return [this.serialiseContent(element.content)];
    }
  }, {
    key: 'enumSerialiseAttributes',
    value: function enumSerialiseAttributes(element) {
      var _this2 = this;

      var attributes = element.attributes.clone();

      // Enumerations attribute was is placed inside content (see `enumSerialiseContent` below)
      var enumerations = attributes.remove('enumerations') || new this.namespace.elements.Array([]);

      // Remove fixed type attribute from samples and default
      var defaultValue = attributes.get('default');
      var samples = attributes.get('samples') || new this.namespace.elements.Array([]);

      if (defaultValue && defaultValue.content) {
        if (defaultValue.content.attributes) {
          defaultValue.content.attributes.remove('typeAttributes');
        }
        // Wrap default in array (not sure it is really needed because tests pass without this line)
        attributes.set('default', new this.namespace.elements.Array([defaultValue.content]));
      }

      // Strip typeAttributes from samples, 0.6 doesn't usually contain them in samples
      samples.forEach(function (sample) {
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

      samples = samples.map(function (sample) {
        if (sample instanceof _this2.namespace.elements.Array) {
          return [sample];
        }

        return new _this2.namespace.elements.Array([sample.content]);
      });

      if (samples.length) {
        attributes.set('samples', samples);
      }

      if (attributes.length > 0) {
        return this.serialiseObject(attributes);
      }

      return undefined;
    }
  }, {
    key: 'enumSerialiseContent',
    value: function enumSerialiseContent(element) {
      var _this3 = this;

      // In API Elements < 1.0, the content is the enumerations
      // If we don't have an enumerations, use the value (Drafter 3 behaviour)

      if (element._attributes) {
        var enumerations = element.attributes.get('enumerations');

        if (enumerations && enumerations.length > 0) {
          return enumerations.content.map(function (enumeration) {
            var e = enumeration.clone();
            e.attributes.remove('typeAttributes');
            return _this3.serialise(e);
          });
        }
      }

      if (element.content) {
        var value = element.content.clone();
        value.attributes.remove('typeAttributes');
        return [this.serialise(value)];
      }

      return [];
    }
  }, {
    key: 'deserialise',
    value: function deserialise(value) {
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

      var ElementClass = this.namespace.getElementClass(value.element);
      var element = new ElementClass();

      if (element.element !== value.element) {
        element.element = value.element;
      }

      if (value.meta) {
        this.deserialiseObject(value.meta, element.meta);
      }

      if (value.attributes) {
        this.deserialiseObject(value.attributes, element.attributes);
      }

      var content = this.deserialiseContent(value.content);
      if (content !== undefined || element.content === null) {
        element.content = content;
      }

      if (element.element === 'enum') {
        // Grab enumerations from content
        if (element.content) {
          element.attributes.set('enumerations', element.content);
        }

        // Unwrap the sample value (inside double array)
        var samples = element.attributes.get('samples');
        element.attributes.remove('samples');

        if (samples) {
          // Re-wrap samples from array of array to array of enum's

          var existingSamples = samples;

          samples = new this.namespace.elements.Array();
          existingSamples.forEach(function (existingSample) {
            existingSample.forEach(function (sample) {
              var enumElement = new ElementClass(sample);
              enumElement.element = element.element;
              samples.push(enumElement);
            });
          });

          var sample = samples.shift();

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
        var defaultValue = element.attributes.get('default');
        if (defaultValue && defaultValue.length > 0) {
          defaultValue = defaultValue.get(0);
          var defaultElement = new ElementClass(defaultValue);
          defaultElement.element = element.element;
          element.attributes.set('default', defaultElement);
        }
      } else if (element.element === 'dataStructure' && Array.isArray(element.content)) {
        var _element$content = _slicedToArray(element.content, 1);

        element.content = _element$content[0];
      } else if (element.element === 'category') {
        // "meta" attribute has been renamed to metadata
        var metadata = element.attributes.get('meta');

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

  }, {
    key: 'serialiseContent',
    value: function serialiseContent(content) {
      if (content instanceof this.namespace.elements.Element) {
        return this.serialise(content);
      }

      if (content instanceof this.namespace.KeyValuePair) {
        var pair = {
          key: this.serialise(content.key)
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
  }, {
    key: 'deserialiseContent',
    value: function deserialiseContent(content) {
      if (content) {
        if (content.element) {
          return this.deserialise(content);
        }

        if (content.key) {
          var pair = new this.namespace.KeyValuePair(this.deserialise(content.key));

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
  }, {
    key: 'shouldRefract',
    value: function shouldRefract(element) {
      if (element._attributes && element.attributes.keys().length || element._meta && element.meta.keys().length) {
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
  }, {
    key: 'convertKeyToRefract',
    value: function convertKeyToRefract(key, item) {
      var _this4 = this;

      if (this.shouldRefract(item)) {
        return this.serialise(item);
      }

      if (item.element === 'enum') {
        return this.serialiseEnum(item);
      }

      if (item.element === 'array') {
        return item.map(function (subItem) {
          if (_this4.shouldRefract(subItem) || key === 'default') {
            return _this4.serialise(subItem);
          }

          if (subItem.element === 'array' || subItem.element === 'object' || subItem.element === 'enum') {
            // items for array or enum inside array are always serialised
            return subItem.children.map(function (subSubItem) {
              return _this4.serialise(subSubItem);
            });
          }

          return subItem.toValue();
        });
      }

      if (item.element === 'object') {
        return (item.content || []).map(this.serialise, this);
      }

      return item.toValue();
    }
  }, {
    key: 'serialiseEnum',
    value: function serialiseEnum(element) {
      var _this5 = this;

      return element.children.map(function (item) {
        return _this5.serialise(item);
      });
    }
  }, {
    key: 'serialiseObject',
    value: function serialiseObject(obj) {
      var _this6 = this;

      var result = {};

      obj.forEach(function (value, key) {
        if (value) {
          var keyValue = key.toValue();
          result[keyValue] = _this6.convertKeyToRefract(keyValue, value);
        }
      });

      return result;
    }
  }, {
    key: 'deserialiseObject',
    value: function deserialiseObject(from, to) {
      var _this7 = this;

      Object.keys(from).forEach(function (key) {
        to.set(key, _this7.deserialise(from[key]));
      });
    }
  }]);

  return JSON06Serialiser;
}(JSONSerialiser);

},{"./JSONSerialiser":18}],18:[function(require,module,exports){
'use strict';

var _createClass = function () {
  function defineProperties(target, props) {
    for (var i = 0; i < props.length; i++) {
      var descriptor = props[i];descriptor.enumerable = descriptor.enumerable || false;descriptor.configurable = true;if ("value" in descriptor) descriptor.writable = true;Object.defineProperty(target, descriptor.key, descriptor);
    }
  }return function (Constructor, protoProps, staticProps) {
    if (protoProps) defineProperties(Constructor.prototype, protoProps);if (staticProps) defineProperties(Constructor, staticProps);return Constructor;
  };
}();

function _classCallCheck(instance, Constructor) {
  if (!(instance instanceof Constructor)) {
    throw new TypeError("Cannot call a class as a function");
  }
}

/**
 * @class JSONSerialiser
 *
 * @param {Namespace} namespace
 *
 * @property {Namespace} namespace
 */
var JSONSerialiser = function () {
  function JSONSerialiser(namespace) {
    _classCallCheck(this, JSONSerialiser);

    this.namespace = namespace || new this.Namespace();
  }

  /**
   * @param {Element} element
   * @returns {object}
   */

  _createClass(JSONSerialiser, [{
    key: 'serialise',
    value: function serialise(element) {
      if (!(element instanceof this.namespace.elements.Element)) {
        throw new TypeError('Given element `' + element + '` is not an Element instance');
      }

      var payload = {
        element: element.element
      };

      if (element._meta && element._meta.length > 0) {
        payload.meta = this.serialiseObject(element.meta);
      }

      if (element._attributes && element._attributes.length > 0) {
        payload.attributes = this.serialiseObject(element.attributes);
      }

      var content = this.serialiseContent(element.content);

      if (content !== undefined) {
        payload.content = content;
      }

      return payload;
    }

    /**
     * @param {object} value
     * @returns {Element}
     */

  }, {
    key: 'deserialise',
    value: function deserialise(value) {
      if (!value.element) {
        throw new Error('Given value is not an object containing an element name');
      }

      var ElementClass = this.namespace.getElementClass(value.element);
      var element = new ElementClass();

      if (element.element !== value.element) {
        element.element = value.element;
      }

      if (value.meta) {
        this.deserialiseObject(value.meta, element.meta);
      }

      if (value.attributes) {
        this.deserialiseObject(value.attributes, element.attributes);
      }

      var content = this.deserialiseContent(value.content);
      if (content !== undefined || element.content === null) {
        element.content = content;
      }

      return element;
    }

    // Private API

  }, {
    key: 'serialiseContent',
    value: function serialiseContent(content) {
      if (content instanceof this.namespace.elements.Element) {
        return this.serialise(content);
      }

      if (content instanceof this.namespace.KeyValuePair) {
        var pair = {
          key: this.serialise(content.key)
        };

        if (content.value) {
          pair.value = this.serialise(content.value);
        }

        return pair;
      }

      if (content && content.map) {
        if (content.length === 0) {
          return undefined;
        }

        return content.map(this.serialise, this);
      }

      return content;
    }
  }, {
    key: 'deserialiseContent',
    value: function deserialiseContent(content) {
      if (content) {
        if (content.element) {
          return this.deserialise(content);
        }

        if (content.key) {
          var pair = new this.namespace.KeyValuePair(this.deserialise(content.key));

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
  }, {
    key: 'serialiseObject',
    value: function serialiseObject(obj) {
      var _this = this;

      var result = {};

      obj.forEach(function (value, key) {
        if (value) {
          result[key.toValue()] = _this.serialise(value);
        }
      });

      if (Object.keys(result).length === 0) {
        return undefined;
      }

      return result;
    }
  }, {
    key: 'deserialiseObject',
    value: function deserialiseObject(from, to) {
      var _this2 = this;

      Object.keys(from).forEach(function (key) {
        to.set(key, _this2.deserialise(from[key]));
      });
    }
  }]);

  return JSONSerialiser;
}();

module.exports = JSONSerialiser;

},{}],19:[function(require,module,exports){
var getNative = require('./_getNative'),
    root = require('./_root');

/* Built-in method references that are verified to be native. */
var DataView = getNative(root, 'DataView');

module.exports = DataView;

},{"./_getNative":54,"./_root":84}],20:[function(require,module,exports){
var hashClear = require('./_hashClear'),
    hashDelete = require('./_hashDelete'),
    hashGet = require('./_hashGet'),
    hashHas = require('./_hashHas'),
    hashSet = require('./_hashSet');

/**
 * Creates a hash object.
 *
 * @private
 * @constructor
 * @param {Array} [entries] The key-value pairs to cache.
 */
function Hash(entries) {
  var index = -1,
      length = entries == null ? 0 : entries.length;

  this.clear();
  while (++index < length) {
    var entry = entries[index];
    this.set(entry[0], entry[1]);
  }
}

// Add methods to `Hash`.
Hash.prototype.clear = hashClear;
Hash.prototype['delete'] = hashDelete;
Hash.prototype.get = hashGet;
Hash.prototype.has = hashHas;
Hash.prototype.set = hashSet;

module.exports = Hash;

},{"./_hashClear":59,"./_hashDelete":60,"./_hashGet":61,"./_hashHas":62,"./_hashSet":63}],21:[function(require,module,exports){
var listCacheClear = require('./_listCacheClear'),
    listCacheDelete = require('./_listCacheDelete'),
    listCacheGet = require('./_listCacheGet'),
    listCacheHas = require('./_listCacheHas'),
    listCacheSet = require('./_listCacheSet');

/**
 * Creates an list cache object.
 *
 * @private
 * @constructor
 * @param {Array} [entries] The key-value pairs to cache.
 */
function ListCache(entries) {
  var index = -1,
      length = entries == null ? 0 : entries.length;

  this.clear();
  while (++index < length) {
    var entry = entries[index];
    this.set(entry[0], entry[1]);
  }
}

// Add methods to `ListCache`.
ListCache.prototype.clear = listCacheClear;
ListCache.prototype['delete'] = listCacheDelete;
ListCache.prototype.get = listCacheGet;
ListCache.prototype.has = listCacheHas;
ListCache.prototype.set = listCacheSet;

module.exports = ListCache;

},{"./_listCacheClear":68,"./_listCacheDelete":69,"./_listCacheGet":70,"./_listCacheHas":71,"./_listCacheSet":72}],22:[function(require,module,exports){
var getNative = require('./_getNative'),
    root = require('./_root');

/* Built-in method references that are verified to be native. */
var Map = getNative(root, 'Map');

module.exports = Map;

},{"./_getNative":54,"./_root":84}],23:[function(require,module,exports){
var mapCacheClear = require('./_mapCacheClear'),
    mapCacheDelete = require('./_mapCacheDelete'),
    mapCacheGet = require('./_mapCacheGet'),
    mapCacheHas = require('./_mapCacheHas'),
    mapCacheSet = require('./_mapCacheSet');

/**
 * Creates a map cache object to store key-value pairs.
 *
 * @private
 * @constructor
 * @param {Array} [entries] The key-value pairs to cache.
 */
function MapCache(entries) {
  var index = -1,
      length = entries == null ? 0 : entries.length;

  this.clear();
  while (++index < length) {
    var entry = entries[index];
    this.set(entry[0], entry[1]);
  }
}

// Add methods to `MapCache`.
MapCache.prototype.clear = mapCacheClear;
MapCache.prototype['delete'] = mapCacheDelete;
MapCache.prototype.get = mapCacheGet;
MapCache.prototype.has = mapCacheHas;
MapCache.prototype.set = mapCacheSet;

module.exports = MapCache;

},{"./_mapCacheClear":73,"./_mapCacheDelete":74,"./_mapCacheGet":75,"./_mapCacheHas":76,"./_mapCacheSet":77}],24:[function(require,module,exports){
var getNative = require('./_getNative'),
    root = require('./_root');

/* Built-in method references that are verified to be native. */
var Promise = getNative(root, 'Promise');

module.exports = Promise;

},{"./_getNative":54,"./_root":84}],25:[function(require,module,exports){
var getNative = require('./_getNative'),
    root = require('./_root');

/* Built-in method references that are verified to be native. */
var Set = getNative(root, 'Set');

module.exports = Set;

},{"./_getNative":54,"./_root":84}],26:[function(require,module,exports){
var MapCache = require('./_MapCache'),
    setCacheAdd = require('./_setCacheAdd'),
    setCacheHas = require('./_setCacheHas');

/**
 *
 * Creates an array cache object to store unique values.
 *
 * @private
 * @constructor
 * @param {Array} [values] The values to cache.
 */
function SetCache(values) {
  var index = -1,
      length = values == null ? 0 : values.length;

  this.__data__ = new MapCache;
  while (++index < length) {
    this.add(values[index]);
  }
}

// Add methods to `SetCache`.
SetCache.prototype.add = SetCache.prototype.push = setCacheAdd;
SetCache.prototype.has = setCacheHas;

module.exports = SetCache;

},{"./_MapCache":23,"./_setCacheAdd":85,"./_setCacheHas":86}],27:[function(require,module,exports){
var ListCache = require('./_ListCache'),
    stackClear = require('./_stackClear'),
    stackDelete = require('./_stackDelete'),
    stackGet = require('./_stackGet'),
    stackHas = require('./_stackHas'),
    stackSet = require('./_stackSet');

/**
 * Creates a stack cache object to store key-value pairs.
 *
 * @private
 * @constructor
 * @param {Array} [entries] The key-value pairs to cache.
 */
function Stack(entries) {
  var data = this.__data__ = new ListCache(entries);
  this.size = data.size;
}

// Add methods to `Stack`.
Stack.prototype.clear = stackClear;
Stack.prototype['delete'] = stackDelete;
Stack.prototype.get = stackGet;
Stack.prototype.has = stackHas;
Stack.prototype.set = stackSet;

module.exports = Stack;

},{"./_ListCache":21,"./_stackClear":88,"./_stackDelete":89,"./_stackGet":90,"./_stackHas":91,"./_stackSet":92}],28:[function(require,module,exports){
var root = require('./_root');

/** Built-in value references. */
var Symbol = root.Symbol;

module.exports = Symbol;

},{"./_root":84}],29:[function(require,module,exports){
var root = require('./_root');

/** Built-in value references. */
var Uint8Array = root.Uint8Array;

module.exports = Uint8Array;

},{"./_root":84}],30:[function(require,module,exports){
var getNative = require('./_getNative'),
    root = require('./_root');

/* Built-in method references that are verified to be native. */
var WeakMap = getNative(root, 'WeakMap');

module.exports = WeakMap;

},{"./_getNative":54,"./_root":84}],31:[function(require,module,exports){
/**
 * A specialized version of `_.filter` for arrays without support for
 * iteratee shorthands.
 *
 * @private
 * @param {Array} [array] The array to iterate over.
 * @param {Function} predicate The function invoked per iteration.
 * @returns {Array} Returns the new filtered array.
 */
function arrayFilter(array, predicate) {
  var index = -1,
      length = array == null ? 0 : array.length,
      resIndex = 0,
      result = [];

  while (++index < length) {
    var value = array[index];
    if (predicate(value, index, array)) {
      result[resIndex++] = value;
    }
  }
  return result;
}

module.exports = arrayFilter;

},{}],32:[function(require,module,exports){
var baseTimes = require('./_baseTimes'),
    isArguments = require('./isArguments'),
    isArray = require('./isArray'),
    isBuffer = require('./isBuffer'),
    isIndex = require('./_isIndex'),
    isTypedArray = require('./isTypedArray');

/** Used for built-in method references. */
var objectProto = Object.prototype;

/** Used to check objects for own properties. */
var hasOwnProperty = objectProto.hasOwnProperty;

/**
 * Creates an array of the enumerable property names of the array-like `value`.
 *
 * @private
 * @param {*} value The value to query.
 * @param {boolean} inherited Specify returning inherited property names.
 * @returns {Array} Returns the array of property names.
 */
function arrayLikeKeys(value, inherited) {
  var isArr = isArray(value),
      isArg = !isArr && isArguments(value),
      isBuff = !isArr && !isArg && isBuffer(value),
      isType = !isArr && !isArg && !isBuff && isTypedArray(value),
      skipIndexes = isArr || isArg || isBuff || isType,
      result = skipIndexes ? baseTimes(value.length, String) : [],
      length = result.length;

  for (var key in value) {
    if ((inherited || hasOwnProperty.call(value, key)) &&
        !(skipIndexes && (
           // Safari 9 has enumerable `arguments.length` in strict mode.
           key == 'length' ||
           // Node.js 0.10 has enumerable non-index properties on buffers.
           (isBuff && (key == 'offset' || key == 'parent')) ||
           // PhantomJS 2 has enumerable non-index properties on typed arrays.
           (isType && (key == 'buffer' || key == 'byteLength' || key == 'byteOffset')) ||
           // Skip index properties.
           isIndex(key, length)
        ))) {
      result.push(key);
    }
  }
  return result;
}

module.exports = arrayLikeKeys;

},{"./_baseTimes":44,"./_isIndex":64,"./isArguments":95,"./isArray":96,"./isBuffer":99,"./isTypedArray":108}],33:[function(require,module,exports){
/**
 * Appends the elements of `values` to `array`.
 *
 * @private
 * @param {Array} array The array to modify.
 * @param {Array} values The values to append.
 * @returns {Array} Returns `array`.
 */
function arrayPush(array, values) {
  var index = -1,
      length = values.length,
      offset = array.length;

  while (++index < length) {
    array[offset + index] = values[index];
  }
  return array;
}

module.exports = arrayPush;

},{}],34:[function(require,module,exports){
/**
 * A specialized version of `_.some` for arrays without support for iteratee
 * shorthands.
 *
 * @private
 * @param {Array} [array] The array to iterate over.
 * @param {Function} predicate The function invoked per iteration.
 * @returns {boolean} Returns `true` if any element passes the predicate check,
 *  else `false`.
 */
function arraySome(array, predicate) {
  var index = -1,
      length = array == null ? 0 : array.length;

  while (++index < length) {
    if (predicate(array[index], index, array)) {
      return true;
    }
  }
  return false;
}

module.exports = arraySome;

},{}],35:[function(require,module,exports){
var eq = require('./eq');

/**
 * Gets the index at which the `key` is found in `array` of key-value pairs.
 *
 * @private
 * @param {Array} array The array to inspect.
 * @param {*} key The key to search for.
 * @returns {number} Returns the index of the matched value, else `-1`.
 */
function assocIndexOf(array, key) {
  var length = array.length;
  while (length--) {
    if (eq(array[length][0], key)) {
      return length;
    }
  }
  return -1;
}

module.exports = assocIndexOf;

},{"./eq":94}],36:[function(require,module,exports){
var arrayPush = require('./_arrayPush'),
    isArray = require('./isArray');

/**
 * The base implementation of `getAllKeys` and `getAllKeysIn` which uses
 * `keysFunc` and `symbolsFunc` to get the enumerable property names and
 * symbols of `object`.
 *
 * @private
 * @param {Object} object The object to query.
 * @param {Function} keysFunc The function to get the keys of `object`.
 * @param {Function} symbolsFunc The function to get the symbols of `object`.
 * @returns {Array} Returns the array of property names and symbols.
 */
function baseGetAllKeys(object, keysFunc, symbolsFunc) {
  var result = keysFunc(object);
  return isArray(object) ? result : arrayPush(result, symbolsFunc(object));
}

module.exports = baseGetAllKeys;

},{"./_arrayPush":33,"./isArray":96}],37:[function(require,module,exports){
var Symbol = require('./_Symbol'),
    getRawTag = require('./_getRawTag'),
    objectToString = require('./_objectToString');

/** `Object#toString` result references. */
var nullTag = '[object Null]',
    undefinedTag = '[object Undefined]';

/** Built-in value references. */
var symToStringTag = Symbol ? Symbol.toStringTag : undefined;

/**
 * The base implementation of `getTag` without fallbacks for buggy environments.
 *
 * @private
 * @param {*} value The value to query.
 * @returns {string} Returns the `toStringTag`.
 */
function baseGetTag(value) {
  if (value == null) {
    return value === undefined ? undefinedTag : nullTag;
  }
  return (symToStringTag && symToStringTag in Object(value))
    ? getRawTag(value)
    : objectToString(value);
}

module.exports = baseGetTag;

},{"./_Symbol":28,"./_getRawTag":55,"./_objectToString":82}],38:[function(require,module,exports){
var baseGetTag = require('./_baseGetTag'),
    isObjectLike = require('./isObjectLike');

/** `Object#toString` result references. */
var argsTag = '[object Arguments]';

/**
 * The base implementation of `_.isArguments`.
 *
 * @private
 * @param {*} value The value to check.
 * @returns {boolean} Returns `true` if `value` is an `arguments` object,
 */
function baseIsArguments(value) {
  return isObjectLike(value) && baseGetTag(value) == argsTag;
}

module.exports = baseIsArguments;

},{"./_baseGetTag":37,"./isObjectLike":106}],39:[function(require,module,exports){
var baseIsEqualDeep = require('./_baseIsEqualDeep'),
    isObjectLike = require('./isObjectLike');

/**
 * The base implementation of `_.isEqual` which supports partial comparisons
 * and tracks traversed objects.
 *
 * @private
 * @param {*} value The value to compare.
 * @param {*} other The other value to compare.
 * @param {boolean} bitmask The bitmask flags.
 *  1 - Unordered comparison
 *  2 - Partial comparison
 * @param {Function} [customizer] The function to customize comparisons.
 * @param {Object} [stack] Tracks traversed `value` and `other` objects.
 * @returns {boolean} Returns `true` if the values are equivalent, else `false`.
 */
function baseIsEqual(value, other, bitmask, customizer, stack) {
  if (value === other) {
    return true;
  }
  if (value == null || other == null || (!isObjectLike(value) && !isObjectLike(other))) {
    return value !== value && other !== other;
  }
  return baseIsEqualDeep(value, other, bitmask, customizer, baseIsEqual, stack);
}

module.exports = baseIsEqual;

},{"./_baseIsEqualDeep":40,"./isObjectLike":106}],40:[function(require,module,exports){
var Stack = require('./_Stack'),
    equalArrays = require('./_equalArrays'),
    equalByTag = require('./_equalByTag'),
    equalObjects = require('./_equalObjects'),
    getTag = require('./_getTag'),
    isArray = require('./isArray'),
    isBuffer = require('./isBuffer'),
    isTypedArray = require('./isTypedArray');

/** Used to compose bitmasks for value comparisons. */
var COMPARE_PARTIAL_FLAG = 1;

/** `Object#toString` result references. */
var argsTag = '[object Arguments]',
    arrayTag = '[object Array]',
    objectTag = '[object Object]';

/** Used for built-in method references. */
var objectProto = Object.prototype;

/** Used to check objects for own properties. */
var hasOwnProperty = objectProto.hasOwnProperty;

/**
 * A specialized version of `baseIsEqual` for arrays and objects which performs
 * deep comparisons and tracks traversed objects enabling objects with circular
 * references to be compared.
 *
 * @private
 * @param {Object} object The object to compare.
 * @param {Object} other The other object to compare.
 * @param {number} bitmask The bitmask flags. See `baseIsEqual` for more details.
 * @param {Function} customizer The function to customize comparisons.
 * @param {Function} equalFunc The function to determine equivalents of values.
 * @param {Object} [stack] Tracks traversed `object` and `other` objects.
 * @returns {boolean} Returns `true` if the objects are equivalent, else `false`.
 */
function baseIsEqualDeep(object, other, bitmask, customizer, equalFunc, stack) {
  var objIsArr = isArray(object),
      othIsArr = isArray(other),
      objTag = objIsArr ? arrayTag : getTag(object),
      othTag = othIsArr ? arrayTag : getTag(other);

  objTag = objTag == argsTag ? objectTag : objTag;
  othTag = othTag == argsTag ? objectTag : othTag;

  var objIsObj = objTag == objectTag,
      othIsObj = othTag == objectTag,
      isSameTag = objTag == othTag;

  if (isSameTag && isBuffer(object)) {
    if (!isBuffer(other)) {
      return false;
    }
    objIsArr = true;
    objIsObj = false;
  }
  if (isSameTag && !objIsObj) {
    stack || (stack = new Stack);
    return (objIsArr || isTypedArray(object))
      ? equalArrays(object, other, bitmask, customizer, equalFunc, stack)
      : equalByTag(object, other, objTag, bitmask, customizer, equalFunc, stack);
  }
  if (!(bitmask & COMPARE_PARTIAL_FLAG)) {
    var objIsWrapped = objIsObj && hasOwnProperty.call(object, '__wrapped__'),
        othIsWrapped = othIsObj && hasOwnProperty.call(other, '__wrapped__');

    if (objIsWrapped || othIsWrapped) {
      var objUnwrapped = objIsWrapped ? object.value() : object,
          othUnwrapped = othIsWrapped ? other.value() : other;

      stack || (stack = new Stack);
      return equalFunc(objUnwrapped, othUnwrapped, bitmask, customizer, stack);
    }
  }
  if (!isSameTag) {
    return false;
  }
  stack || (stack = new Stack);
  return equalObjects(object, other, bitmask, customizer, equalFunc, stack);
}

module.exports = baseIsEqualDeep;

},{"./_Stack":27,"./_equalArrays":48,"./_equalByTag":49,"./_equalObjects":50,"./_getTag":57,"./isArray":96,"./isBuffer":99,"./isTypedArray":108}],41:[function(require,module,exports){
var isFunction = require('./isFunction'),
    isMasked = require('./_isMasked'),
    isObject = require('./isObject'),
    toSource = require('./_toSource');

/**
 * Used to match `RegExp`
 * [syntax characters](http://ecma-international.org/ecma-262/7.0/#sec-patterns).
 */
var reRegExpChar = /[\\^$.*+?()[\]{}|]/g;

/** Used to detect host constructors (Safari). */
var reIsHostCtor = /^\[object .+?Constructor\]$/;

/** Used for built-in method references. */
var funcProto = Function.prototype,
    objectProto = Object.prototype;

/** Used to resolve the decompiled source of functions. */
var funcToString = funcProto.toString;

/** Used to check objects for own properties. */
var hasOwnProperty = objectProto.hasOwnProperty;

/** Used to detect if a method is native. */
var reIsNative = RegExp('^' +
  funcToString.call(hasOwnProperty).replace(reRegExpChar, '\\$&')
  .replace(/hasOwnProperty|(function).*?(?=\\\()| for .+?(?=\\\])/g, '$1.*?') + '$'
);

/**
 * The base implementation of `_.isNative` without bad shim checks.
 *
 * @private
 * @param {*} value The value to check.
 * @returns {boolean} Returns `true` if `value` is a native function,
 *  else `false`.
 */
function baseIsNative(value) {
  if (!isObject(value) || isMasked(value)) {
    return false;
  }
  var pattern = isFunction(value) ? reIsNative : reIsHostCtor;
  return pattern.test(toSource(value));
}

module.exports = baseIsNative;

},{"./_isMasked":66,"./_toSource":93,"./isFunction":101,"./isObject":105}],42:[function(require,module,exports){
var baseGetTag = require('./_baseGetTag'),
    isLength = require('./isLength'),
    isObjectLike = require('./isObjectLike');

/** `Object#toString` result references. */
var argsTag = '[object Arguments]',
    arrayTag = '[object Array]',
    boolTag = '[object Boolean]',
    dateTag = '[object Date]',
    errorTag = '[object Error]',
    funcTag = '[object Function]',
    mapTag = '[object Map]',
    numberTag = '[object Number]',
    objectTag = '[object Object]',
    regexpTag = '[object RegExp]',
    setTag = '[object Set]',
    stringTag = '[object String]',
    weakMapTag = '[object WeakMap]';

var arrayBufferTag = '[object ArrayBuffer]',
    dataViewTag = '[object DataView]',
    float32Tag = '[object Float32Array]',
    float64Tag = '[object Float64Array]',
    int8Tag = '[object Int8Array]',
    int16Tag = '[object Int16Array]',
    int32Tag = '[object Int32Array]',
    uint8Tag = '[object Uint8Array]',
    uint8ClampedTag = '[object Uint8ClampedArray]',
    uint16Tag = '[object Uint16Array]',
    uint32Tag = '[object Uint32Array]';

/** Used to identify `toStringTag` values of typed arrays. */
var typedArrayTags = {};
typedArrayTags[float32Tag] = typedArrayTags[float64Tag] =
typedArrayTags[int8Tag] = typedArrayTags[int16Tag] =
typedArrayTags[int32Tag] = typedArrayTags[uint8Tag] =
typedArrayTags[uint8ClampedTag] = typedArrayTags[uint16Tag] =
typedArrayTags[uint32Tag] = true;
typedArrayTags[argsTag] = typedArrayTags[arrayTag] =
typedArrayTags[arrayBufferTag] = typedArrayTags[boolTag] =
typedArrayTags[dataViewTag] = typedArrayTags[dateTag] =
typedArrayTags[errorTag] = typedArrayTags[funcTag] =
typedArrayTags[mapTag] = typedArrayTags[numberTag] =
typedArrayTags[objectTag] = typedArrayTags[regexpTag] =
typedArrayTags[setTag] = typedArrayTags[stringTag] =
typedArrayTags[weakMapTag] = false;

/**
 * The base implementation of `_.isTypedArray` without Node.js optimizations.
 *
 * @private
 * @param {*} value The value to check.
 * @returns {boolean} Returns `true` if `value` is a typed array, else `false`.
 */
function baseIsTypedArray(value) {
  return isObjectLike(value) &&
    isLength(value.length) && !!typedArrayTags[baseGetTag(value)];
}

module.exports = baseIsTypedArray;

},{"./_baseGetTag":37,"./isLength":102,"./isObjectLike":106}],43:[function(require,module,exports){
var isPrototype = require('./_isPrototype'),
    nativeKeys = require('./_nativeKeys');

/** Used for built-in method references. */
var objectProto = Object.prototype;

/** Used to check objects for own properties. */
var hasOwnProperty = objectProto.hasOwnProperty;

/**
 * The base implementation of `_.keys` which doesn't treat sparse arrays as dense.
 *
 * @private
 * @param {Object} object The object to query.
 * @returns {Array} Returns the array of property names.
 */
function baseKeys(object) {
  if (!isPrototype(object)) {
    return nativeKeys(object);
  }
  var result = [];
  for (var key in Object(object)) {
    if (hasOwnProperty.call(object, key) && key != 'constructor') {
      result.push(key);
    }
  }
  return result;
}

module.exports = baseKeys;

},{"./_isPrototype":67,"./_nativeKeys":80}],44:[function(require,module,exports){
/**
 * The base implementation of `_.times` without support for iteratee shorthands
 * or max array length checks.
 *
 * @private
 * @param {number} n The number of times to invoke `iteratee`.
 * @param {Function} iteratee The function invoked per iteration.
 * @returns {Array} Returns the array of results.
 */
function baseTimes(n, iteratee) {
  var index = -1,
      result = Array(n);

  while (++index < n) {
    result[index] = iteratee(index);
  }
  return result;
}

module.exports = baseTimes;

},{}],45:[function(require,module,exports){
/**
 * The base implementation of `_.unary` without support for storing metadata.
 *
 * @private
 * @param {Function} func The function to cap arguments for.
 * @returns {Function} Returns the new capped function.
 */
function baseUnary(func) {
  return function(value) {
    return func(value);
  };
}

module.exports = baseUnary;

},{}],46:[function(require,module,exports){
/**
 * Checks if a `cache` value for `key` exists.
 *
 * @private
 * @param {Object} cache The cache to query.
 * @param {string} key The key of the entry to check.
 * @returns {boolean} Returns `true` if an entry for `key` exists, else `false`.
 */
function cacheHas(cache, key) {
  return cache.has(key);
}

module.exports = cacheHas;

},{}],47:[function(require,module,exports){
var root = require('./_root');

/** Used to detect overreaching core-js shims. */
var coreJsData = root['__core-js_shared__'];

module.exports = coreJsData;

},{"./_root":84}],48:[function(require,module,exports){
var SetCache = require('./_SetCache'),
    arraySome = require('./_arraySome'),
    cacheHas = require('./_cacheHas');

/** Used to compose bitmasks for value comparisons. */
var COMPARE_PARTIAL_FLAG = 1,
    COMPARE_UNORDERED_FLAG = 2;

/**
 * A specialized version of `baseIsEqualDeep` for arrays with support for
 * partial deep comparisons.
 *
 * @private
 * @param {Array} array The array to compare.
 * @param {Array} other The other array to compare.
 * @param {number} bitmask The bitmask flags. See `baseIsEqual` for more details.
 * @param {Function} customizer The function to customize comparisons.
 * @param {Function} equalFunc The function to determine equivalents of values.
 * @param {Object} stack Tracks traversed `array` and `other` objects.
 * @returns {boolean} Returns `true` if the arrays are equivalent, else `false`.
 */
function equalArrays(array, other, bitmask, customizer, equalFunc, stack) {
  var isPartial = bitmask & COMPARE_PARTIAL_FLAG,
      arrLength = array.length,
      othLength = other.length;

  if (arrLength != othLength && !(isPartial && othLength > arrLength)) {
    return false;
  }
  // Assume cyclic values are equal.
  var stacked = stack.get(array);
  if (stacked && stack.get(other)) {
    return stacked == other;
  }
  var index = -1,
      result = true,
      seen = (bitmask & COMPARE_UNORDERED_FLAG) ? new SetCache : undefined;

  stack.set(array, other);
  stack.set(other, array);

  // Ignore non-index properties.
  while (++index < arrLength) {
    var arrValue = array[index],
        othValue = other[index];

    if (customizer) {
      var compared = isPartial
        ? customizer(othValue, arrValue, index, other, array, stack)
        : customizer(arrValue, othValue, index, array, other, stack);
    }
    if (compared !== undefined) {
      if (compared) {
        continue;
      }
      result = false;
      break;
    }
    // Recursively compare arrays (susceptible to call stack limits).
    if (seen) {
      if (!arraySome(other, function(othValue, othIndex) {
            if (!cacheHas(seen, othIndex) &&
                (arrValue === othValue || equalFunc(arrValue, othValue, bitmask, customizer, stack))) {
              return seen.push(othIndex);
            }
          })) {
        result = false;
        break;
      }
    } else if (!(
          arrValue === othValue ||
            equalFunc(arrValue, othValue, bitmask, customizer, stack)
        )) {
      result = false;
      break;
    }
  }
  stack['delete'](array);
  stack['delete'](other);
  return result;
}

module.exports = equalArrays;

},{"./_SetCache":26,"./_arraySome":34,"./_cacheHas":46}],49:[function(require,module,exports){
var Symbol = require('./_Symbol'),
    Uint8Array = require('./_Uint8Array'),
    eq = require('./eq'),
    equalArrays = require('./_equalArrays'),
    mapToArray = require('./_mapToArray'),
    setToArray = require('./_setToArray');

/** Used to compose bitmasks for value comparisons. */
var COMPARE_PARTIAL_FLAG = 1,
    COMPARE_UNORDERED_FLAG = 2;

/** `Object#toString` result references. */
var boolTag = '[object Boolean]',
    dateTag = '[object Date]',
    errorTag = '[object Error]',
    mapTag = '[object Map]',
    numberTag = '[object Number]',
    regexpTag = '[object RegExp]',
    setTag = '[object Set]',
    stringTag = '[object String]',
    symbolTag = '[object Symbol]';

var arrayBufferTag = '[object ArrayBuffer]',
    dataViewTag = '[object DataView]';

/** Used to convert symbols to primitives and strings. */
var symbolProto = Symbol ? Symbol.prototype : undefined,
    symbolValueOf = symbolProto ? symbolProto.valueOf : undefined;

/**
 * A specialized version of `baseIsEqualDeep` for comparing objects of
 * the same `toStringTag`.
 *
 * **Note:** This function only supports comparing values with tags of
 * `Boolean`, `Date`, `Error`, `Number`, `RegExp`, or `String`.
 *
 * @private
 * @param {Object} object The object to compare.
 * @param {Object} other The other object to compare.
 * @param {string} tag The `toStringTag` of the objects to compare.
 * @param {number} bitmask The bitmask flags. See `baseIsEqual` for more details.
 * @param {Function} customizer The function to customize comparisons.
 * @param {Function} equalFunc The function to determine equivalents of values.
 * @param {Object} stack Tracks traversed `object` and `other` objects.
 * @returns {boolean} Returns `true` if the objects are equivalent, else `false`.
 */
function equalByTag(object, other, tag, bitmask, customizer, equalFunc, stack) {
  switch (tag) {
    case dataViewTag:
      if ((object.byteLength != other.byteLength) ||
          (object.byteOffset != other.byteOffset)) {
        return false;
      }
      object = object.buffer;
      other = other.buffer;

    case arrayBufferTag:
      if ((object.byteLength != other.byteLength) ||
          !equalFunc(new Uint8Array(object), new Uint8Array(other))) {
        return false;
      }
      return true;

    case boolTag:
    case dateTag:
    case numberTag:
      // Coerce booleans to `1` or `0` and dates to milliseconds.
      // Invalid dates are coerced to `NaN`.
      return eq(+object, +other);

    case errorTag:
      return object.name == other.name && object.message == other.message;

    case regexpTag:
    case stringTag:
      // Coerce regexes to strings and treat strings, primitives and objects,
      // as equal. See http://www.ecma-international.org/ecma-262/7.0/#sec-regexp.prototype.tostring
      // for more details.
      return object == (other + '');

    case mapTag:
      var convert = mapToArray;

    case setTag:
      var isPartial = bitmask & COMPARE_PARTIAL_FLAG;
      convert || (convert = setToArray);

      if (object.size != other.size && !isPartial) {
        return false;
      }
      // Assume cyclic values are equal.
      var stacked = stack.get(object);
      if (stacked) {
        return stacked == other;
      }
      bitmask |= COMPARE_UNORDERED_FLAG;

      // Recursively compare objects (susceptible to call stack limits).
      stack.set(object, other);
      var result = equalArrays(convert(object), convert(other), bitmask, customizer, equalFunc, stack);
      stack['delete'](object);
      return result;

    case symbolTag:
      if (symbolValueOf) {
        return symbolValueOf.call(object) == symbolValueOf.call(other);
      }
  }
  return false;
}

module.exports = equalByTag;

},{"./_Symbol":28,"./_Uint8Array":29,"./_equalArrays":48,"./_mapToArray":78,"./_setToArray":87,"./eq":94}],50:[function(require,module,exports){
var getAllKeys = require('./_getAllKeys');

/** Used to compose bitmasks for value comparisons. */
var COMPARE_PARTIAL_FLAG = 1;

/** Used for built-in method references. */
var objectProto = Object.prototype;

/** Used to check objects for own properties. */
var hasOwnProperty = objectProto.hasOwnProperty;

/**
 * A specialized version of `baseIsEqualDeep` for objects with support for
 * partial deep comparisons.
 *
 * @private
 * @param {Object} object The object to compare.
 * @param {Object} other The other object to compare.
 * @param {number} bitmask The bitmask flags. See `baseIsEqual` for more details.
 * @param {Function} customizer The function to customize comparisons.
 * @param {Function} equalFunc The function to determine equivalents of values.
 * @param {Object} stack Tracks traversed `object` and `other` objects.
 * @returns {boolean} Returns `true` if the objects are equivalent, else `false`.
 */
function equalObjects(object, other, bitmask, customizer, equalFunc, stack) {
  var isPartial = bitmask & COMPARE_PARTIAL_FLAG,
      objProps = getAllKeys(object),
      objLength = objProps.length,
      othProps = getAllKeys(other),
      othLength = othProps.length;

  if (objLength != othLength && !isPartial) {
    return false;
  }
  var index = objLength;
  while (index--) {
    var key = objProps[index];
    if (!(isPartial ? key in other : hasOwnProperty.call(other, key))) {
      return false;
    }
  }
  // Assume cyclic values are equal.
  var stacked = stack.get(object);
  if (stacked && stack.get(other)) {
    return stacked == other;
  }
  var result = true;
  stack.set(object, other);
  stack.set(other, object);

  var skipCtor = isPartial;
  while (++index < objLength) {
    key = objProps[index];
    var objValue = object[key],
        othValue = other[key];

    if (customizer) {
      var compared = isPartial
        ? customizer(othValue, objValue, key, other, object, stack)
        : customizer(objValue, othValue, key, object, other, stack);
    }
    // Recursively compare objects (susceptible to call stack limits).
    if (!(compared === undefined
          ? (objValue === othValue || equalFunc(objValue, othValue, bitmask, customizer, stack))
          : compared
        )) {
      result = false;
      break;
    }
    skipCtor || (skipCtor = key == 'constructor');
  }
  if (result && !skipCtor) {
    var objCtor = object.constructor,
        othCtor = other.constructor;

    // Non `Object` object instances with different constructors are not equal.
    if (objCtor != othCtor &&
        ('constructor' in object && 'constructor' in other) &&
        !(typeof objCtor == 'function' && objCtor instanceof objCtor &&
          typeof othCtor == 'function' && othCtor instanceof othCtor)) {
      result = false;
    }
  }
  stack['delete'](object);
  stack['delete'](other);
  return result;
}

module.exports = equalObjects;

},{"./_getAllKeys":52}],51:[function(require,module,exports){
(function (global){
/** Detect free variable `global` from Node.js. */
var freeGlobal = typeof global == 'object' && global && global.Object === Object && global;

module.exports = freeGlobal;

}).call(this,typeof global !== "undefined" ? global : typeof self !== "undefined" ? self : typeof window !== "undefined" ? window : {})

},{}],52:[function(require,module,exports){
var baseGetAllKeys = require('./_baseGetAllKeys'),
    getSymbols = require('./_getSymbols'),
    keys = require('./keys');

/**
 * Creates an array of own enumerable property names and symbols of `object`.
 *
 * @private
 * @param {Object} object The object to query.
 * @returns {Array} Returns the array of property names and symbols.
 */
function getAllKeys(object) {
  return baseGetAllKeys(object, keys, getSymbols);
}

module.exports = getAllKeys;

},{"./_baseGetAllKeys":36,"./_getSymbols":56,"./keys":109}],53:[function(require,module,exports){
var isKeyable = require('./_isKeyable');

/**
 * Gets the data for `map`.
 *
 * @private
 * @param {Object} map The map to query.
 * @param {string} key The reference key.
 * @returns {*} Returns the map data.
 */
function getMapData(map, key) {
  var data = map.__data__;
  return isKeyable(key)
    ? data[typeof key == 'string' ? 'string' : 'hash']
    : data.map;
}

module.exports = getMapData;

},{"./_isKeyable":65}],54:[function(require,module,exports){
var baseIsNative = require('./_baseIsNative'),
    getValue = require('./_getValue');

/**
 * Gets the native function at `key` of `object`.
 *
 * @private
 * @param {Object} object The object to query.
 * @param {string} key The key of the method to get.
 * @returns {*} Returns the function if it's native, else `undefined`.
 */
function getNative(object, key) {
  var value = getValue(object, key);
  return baseIsNative(value) ? value : undefined;
}

module.exports = getNative;

},{"./_baseIsNative":41,"./_getValue":58}],55:[function(require,module,exports){
var Symbol = require('./_Symbol');

/** Used for built-in method references. */
var objectProto = Object.prototype;

/** Used to check objects for own properties. */
var hasOwnProperty = objectProto.hasOwnProperty;

/**
 * Used to resolve the
 * [`toStringTag`](http://ecma-international.org/ecma-262/7.0/#sec-object.prototype.tostring)
 * of values.
 */
var nativeObjectToString = objectProto.toString;

/** Built-in value references. */
var symToStringTag = Symbol ? Symbol.toStringTag : undefined;

/**
 * A specialized version of `baseGetTag` which ignores `Symbol.toStringTag` values.
 *
 * @private
 * @param {*} value The value to query.
 * @returns {string} Returns the raw `toStringTag`.
 */
function getRawTag(value) {
  var isOwn = hasOwnProperty.call(value, symToStringTag),
      tag = value[symToStringTag];

  try {
    value[symToStringTag] = undefined;
    var unmasked = true;
  } catch (e) {}

  var result = nativeObjectToString.call(value);
  if (unmasked) {
    if (isOwn) {
      value[symToStringTag] = tag;
    } else {
      delete value[symToStringTag];
    }
  }
  return result;
}

module.exports = getRawTag;

},{"./_Symbol":28}],56:[function(require,module,exports){
var arrayFilter = require('./_arrayFilter'),
    stubArray = require('./stubArray');

/** Used for built-in method references. */
var objectProto = Object.prototype;

/** Built-in value references. */
var propertyIsEnumerable = objectProto.propertyIsEnumerable;

/* Built-in method references for those with the same name as other `lodash` methods. */
var nativeGetSymbols = Object.getOwnPropertySymbols;

/**
 * Creates an array of the own enumerable symbols of `object`.
 *
 * @private
 * @param {Object} object The object to query.
 * @returns {Array} Returns the array of symbols.
 */
var getSymbols = !nativeGetSymbols ? stubArray : function(object) {
  if (object == null) {
    return [];
  }
  object = Object(object);
  return arrayFilter(nativeGetSymbols(object), function(symbol) {
    return propertyIsEnumerable.call(object, symbol);
  });
};

module.exports = getSymbols;

},{"./_arrayFilter":31,"./stubArray":111}],57:[function(require,module,exports){
var DataView = require('./_DataView'),
    Map = require('./_Map'),
    Promise = require('./_Promise'),
    Set = require('./_Set'),
    WeakMap = require('./_WeakMap'),
    baseGetTag = require('./_baseGetTag'),
    toSource = require('./_toSource');

/** `Object#toString` result references. */
var mapTag = '[object Map]',
    objectTag = '[object Object]',
    promiseTag = '[object Promise]',
    setTag = '[object Set]',
    weakMapTag = '[object WeakMap]';

var dataViewTag = '[object DataView]';

/** Used to detect maps, sets, and weakmaps. */
var dataViewCtorString = toSource(DataView),
    mapCtorString = toSource(Map),
    promiseCtorString = toSource(Promise),
    setCtorString = toSource(Set),
    weakMapCtorString = toSource(WeakMap);

/**
 * Gets the `toStringTag` of `value`.
 *
 * @private
 * @param {*} value The value to query.
 * @returns {string} Returns the `toStringTag`.
 */
var getTag = baseGetTag;

// Fallback for data views, maps, sets, and weak maps in IE 11 and promises in Node.js < 6.
if ((DataView && getTag(new DataView(new ArrayBuffer(1))) != dataViewTag) ||
    (Map && getTag(new Map) != mapTag) ||
    (Promise && getTag(Promise.resolve()) != promiseTag) ||
    (Set && getTag(new Set) != setTag) ||
    (WeakMap && getTag(new WeakMap) != weakMapTag)) {
  getTag = function(value) {
    var result = baseGetTag(value),
        Ctor = result == objectTag ? value.constructor : undefined,
        ctorString = Ctor ? toSource(Ctor) : '';

    if (ctorString) {
      switch (ctorString) {
        case dataViewCtorString: return dataViewTag;
        case mapCtorString: return mapTag;
        case promiseCtorString: return promiseTag;
        case setCtorString: return setTag;
        case weakMapCtorString: return weakMapTag;
      }
    }
    return result;
  };
}

module.exports = getTag;

},{"./_DataView":19,"./_Map":22,"./_Promise":24,"./_Set":25,"./_WeakMap":30,"./_baseGetTag":37,"./_toSource":93}],58:[function(require,module,exports){
/**
 * Gets the value at `key` of `object`.
 *
 * @private
 * @param {Object} [object] The object to query.
 * @param {string} key The key of the property to get.
 * @returns {*} Returns the property value.
 */
function getValue(object, key) {
  return object == null ? undefined : object[key];
}

module.exports = getValue;

},{}],59:[function(require,module,exports){
var nativeCreate = require('./_nativeCreate');

/**
 * Removes all key-value entries from the hash.
 *
 * @private
 * @name clear
 * @memberOf Hash
 */
function hashClear() {
  this.__data__ = nativeCreate ? nativeCreate(null) : {};
  this.size = 0;
}

module.exports = hashClear;

},{"./_nativeCreate":79}],60:[function(require,module,exports){
/**
 * Removes `key` and its value from the hash.
 *
 * @private
 * @name delete
 * @memberOf Hash
 * @param {Object} hash The hash to modify.
 * @param {string} key The key of the value to remove.
 * @returns {boolean} Returns `true` if the entry was removed, else `false`.
 */
function hashDelete(key) {
  var result = this.has(key) && delete this.__data__[key];
  this.size -= result ? 1 : 0;
  return result;
}

module.exports = hashDelete;

},{}],61:[function(require,module,exports){
var nativeCreate = require('./_nativeCreate');

/** Used to stand-in for `undefined` hash values. */
var HASH_UNDEFINED = '__lodash_hash_undefined__';

/** Used for built-in method references. */
var objectProto = Object.prototype;

/** Used to check objects for own properties. */
var hasOwnProperty = objectProto.hasOwnProperty;

/**
 * Gets the hash value for `key`.
 *
 * @private
 * @name get
 * @memberOf Hash
 * @param {string} key The key of the value to get.
 * @returns {*} Returns the entry value.
 */
function hashGet(key) {
  var data = this.__data__;
  if (nativeCreate) {
    var result = data[key];
    return result === HASH_UNDEFINED ? undefined : result;
  }
  return hasOwnProperty.call(data, key) ? data[key] : undefined;
}

module.exports = hashGet;

},{"./_nativeCreate":79}],62:[function(require,module,exports){
var nativeCreate = require('./_nativeCreate');

/** Used for built-in method references. */
var objectProto = Object.prototype;

/** Used to check objects for own properties. */
var hasOwnProperty = objectProto.hasOwnProperty;

/**
 * Checks if a hash value for `key` exists.
 *
 * @private
 * @name has
 * @memberOf Hash
 * @param {string} key The key of the entry to check.
 * @returns {boolean} Returns `true` if an entry for `key` exists, else `false`.
 */
function hashHas(key) {
  var data = this.__data__;
  return nativeCreate ? (data[key] !== undefined) : hasOwnProperty.call(data, key);
}

module.exports = hashHas;

},{"./_nativeCreate":79}],63:[function(require,module,exports){
var nativeCreate = require('./_nativeCreate');

/** Used to stand-in for `undefined` hash values. */
var HASH_UNDEFINED = '__lodash_hash_undefined__';

/**
 * Sets the hash `key` to `value`.
 *
 * @private
 * @name set
 * @memberOf Hash
 * @param {string} key The key of the value to set.
 * @param {*} value The value to set.
 * @returns {Object} Returns the hash instance.
 */
function hashSet(key, value) {
  var data = this.__data__;
  this.size += this.has(key) ? 0 : 1;
  data[key] = (nativeCreate && value === undefined) ? HASH_UNDEFINED : value;
  return this;
}

module.exports = hashSet;

},{"./_nativeCreate":79}],64:[function(require,module,exports){
/** Used as references for various `Number` constants. */
var MAX_SAFE_INTEGER = 9007199254740991;

/** Used to detect unsigned integer values. */
var reIsUint = /^(?:0|[1-9]\d*)$/;

/**
 * Checks if `value` is a valid array-like index.
 *
 * @private
 * @param {*} value The value to check.
 * @param {number} [length=MAX_SAFE_INTEGER] The upper bounds of a valid index.
 * @returns {boolean} Returns `true` if `value` is a valid index, else `false`.
 */
function isIndex(value, length) {
  var type = typeof value;
  length = length == null ? MAX_SAFE_INTEGER : length;

  return !!length &&
    (type == 'number' ||
      (type != 'symbol' && reIsUint.test(value))) &&
        (value > -1 && value % 1 == 0 && value < length);
}

module.exports = isIndex;

},{}],65:[function(require,module,exports){
/**
 * Checks if `value` is suitable for use as unique object key.
 *
 * @private
 * @param {*} value The value to check.
 * @returns {boolean} Returns `true` if `value` is suitable, else `false`.
 */
function isKeyable(value) {
  var type = typeof value;
  return (type == 'string' || type == 'number' || type == 'symbol' || type == 'boolean')
    ? (value !== '__proto__')
    : (value === null);
}

module.exports = isKeyable;

},{}],66:[function(require,module,exports){
var coreJsData = require('./_coreJsData');

/** Used to detect methods masquerading as native. */
var maskSrcKey = (function() {
  var uid = /[^.]+$/.exec(coreJsData && coreJsData.keys && coreJsData.keys.IE_PROTO || '');
  return uid ? ('Symbol(src)_1.' + uid) : '';
}());

/**
 * Checks if `func` has its source masked.
 *
 * @private
 * @param {Function} func The function to check.
 * @returns {boolean} Returns `true` if `func` is masked, else `false`.
 */
function isMasked(func) {
  return !!maskSrcKey && (maskSrcKey in func);
}

module.exports = isMasked;

},{"./_coreJsData":47}],67:[function(require,module,exports){
/** Used for built-in method references. */
var objectProto = Object.prototype;

/**
 * Checks if `value` is likely a prototype object.
 *
 * @private
 * @param {*} value The value to check.
 * @returns {boolean} Returns `true` if `value` is a prototype, else `false`.
 */
function isPrototype(value) {
  var Ctor = value && value.constructor,
      proto = (typeof Ctor == 'function' && Ctor.prototype) || objectProto;

  return value === proto;
}

module.exports = isPrototype;

},{}],68:[function(require,module,exports){
/**
 * Removes all key-value entries from the list cache.
 *
 * @private
 * @name clear
 * @memberOf ListCache
 */
function listCacheClear() {
  this.__data__ = [];
  this.size = 0;
}

module.exports = listCacheClear;

},{}],69:[function(require,module,exports){
var assocIndexOf = require('./_assocIndexOf');

/** Used for built-in method references. */
var arrayProto = Array.prototype;

/** Built-in value references. */
var splice = arrayProto.splice;

/**
 * Removes `key` and its value from the list cache.
 *
 * @private
 * @name delete
 * @memberOf ListCache
 * @param {string} key The key of the value to remove.
 * @returns {boolean} Returns `true` if the entry was removed, else `false`.
 */
function listCacheDelete(key) {
  var data = this.__data__,
      index = assocIndexOf(data, key);

  if (index < 0) {
    return false;
  }
  var lastIndex = data.length - 1;
  if (index == lastIndex) {
    data.pop();
  } else {
    splice.call(data, index, 1);
  }
  --this.size;
  return true;
}

module.exports = listCacheDelete;

},{"./_assocIndexOf":35}],70:[function(require,module,exports){
var assocIndexOf = require('./_assocIndexOf');

/**
 * Gets the list cache value for `key`.
 *
 * @private
 * @name get
 * @memberOf ListCache
 * @param {string} key The key of the value to get.
 * @returns {*} Returns the entry value.
 */
function listCacheGet(key) {
  var data = this.__data__,
      index = assocIndexOf(data, key);

  return index < 0 ? undefined : data[index][1];
}

module.exports = listCacheGet;

},{"./_assocIndexOf":35}],71:[function(require,module,exports){
var assocIndexOf = require('./_assocIndexOf');

/**
 * Checks if a list cache value for `key` exists.
 *
 * @private
 * @name has
 * @memberOf ListCache
 * @param {string} key The key of the entry to check.
 * @returns {boolean} Returns `true` if an entry for `key` exists, else `false`.
 */
function listCacheHas(key) {
  return assocIndexOf(this.__data__, key) > -1;
}

module.exports = listCacheHas;

},{"./_assocIndexOf":35}],72:[function(require,module,exports){
var assocIndexOf = require('./_assocIndexOf');

/**
 * Sets the list cache `key` to `value`.
 *
 * @private
 * @name set
 * @memberOf ListCache
 * @param {string} key The key of the value to set.
 * @param {*} value The value to set.
 * @returns {Object} Returns the list cache instance.
 */
function listCacheSet(key, value) {
  var data = this.__data__,
      index = assocIndexOf(data, key);

  if (index < 0) {
    ++this.size;
    data.push([key, value]);
  } else {
    data[index][1] = value;
  }
  return this;
}

module.exports = listCacheSet;

},{"./_assocIndexOf":35}],73:[function(require,module,exports){
var Hash = require('./_Hash'),
    ListCache = require('./_ListCache'),
    Map = require('./_Map');

/**
 * Removes all key-value entries from the map.
 *
 * @private
 * @name clear
 * @memberOf MapCache
 */
function mapCacheClear() {
  this.size = 0;
  this.__data__ = {
    'hash': new Hash,
    'map': new (Map || ListCache),
    'string': new Hash
  };
}

module.exports = mapCacheClear;

},{"./_Hash":20,"./_ListCache":21,"./_Map":22}],74:[function(require,module,exports){
var getMapData = require('./_getMapData');

/**
 * Removes `key` and its value from the map.
 *
 * @private
 * @name delete
 * @memberOf MapCache
 * @param {string} key The key of the value to remove.
 * @returns {boolean} Returns `true` if the entry was removed, else `false`.
 */
function mapCacheDelete(key) {
  var result = getMapData(this, key)['delete'](key);
  this.size -= result ? 1 : 0;
  return result;
}

module.exports = mapCacheDelete;

},{"./_getMapData":53}],75:[function(require,module,exports){
var getMapData = require('./_getMapData');

/**
 * Gets the map value for `key`.
 *
 * @private
 * @name get
 * @memberOf MapCache
 * @param {string} key The key of the value to get.
 * @returns {*} Returns the entry value.
 */
function mapCacheGet(key) {
  return getMapData(this, key).get(key);
}

module.exports = mapCacheGet;

},{"./_getMapData":53}],76:[function(require,module,exports){
var getMapData = require('./_getMapData');

/**
 * Checks if a map value for `key` exists.
 *
 * @private
 * @name has
 * @memberOf MapCache
 * @param {string} key The key of the entry to check.
 * @returns {boolean} Returns `true` if an entry for `key` exists, else `false`.
 */
function mapCacheHas(key) {
  return getMapData(this, key).has(key);
}

module.exports = mapCacheHas;

},{"./_getMapData":53}],77:[function(require,module,exports){
var getMapData = require('./_getMapData');

/**
 * Sets the map `key` to `value`.
 *
 * @private
 * @name set
 * @memberOf MapCache
 * @param {string} key The key of the value to set.
 * @param {*} value The value to set.
 * @returns {Object} Returns the map cache instance.
 */
function mapCacheSet(key, value) {
  var data = getMapData(this, key),
      size = data.size;

  data.set(key, value);
  this.size += data.size == size ? 0 : 1;
  return this;
}

module.exports = mapCacheSet;

},{"./_getMapData":53}],78:[function(require,module,exports){
/**
 * Converts `map` to its key-value pairs.
 *
 * @private
 * @param {Object} map The map to convert.
 * @returns {Array} Returns the key-value pairs.
 */
function mapToArray(map) {
  var index = -1,
      result = Array(map.size);

  map.forEach(function(value, key) {
    result[++index] = [key, value];
  });
  return result;
}

module.exports = mapToArray;

},{}],79:[function(require,module,exports){
var getNative = require('./_getNative');

/* Built-in method references that are verified to be native. */
var nativeCreate = getNative(Object, 'create');

module.exports = nativeCreate;

},{"./_getNative":54}],80:[function(require,module,exports){
var overArg = require('./_overArg');

/* Built-in method references for those with the same name as other `lodash` methods. */
var nativeKeys = overArg(Object.keys, Object);

module.exports = nativeKeys;

},{"./_overArg":83}],81:[function(require,module,exports){
var freeGlobal = require('./_freeGlobal');

/** Detect free variable `exports`. */
var freeExports = typeof exports == 'object' && exports && !exports.nodeType && exports;

/** Detect free variable `module`. */
var freeModule = freeExports && typeof module == 'object' && module && !module.nodeType && module;

/** Detect the popular CommonJS extension `module.exports`. */
var moduleExports = freeModule && freeModule.exports === freeExports;

/** Detect free variable `process` from Node.js. */
var freeProcess = moduleExports && freeGlobal.process;

/** Used to access faster Node.js helpers. */
var nodeUtil = (function() {
  try {
    // Use `util.types` for Node.js 10+.
    var types = freeModule && freeModule.require && freeModule.require('util').types;

    if (types) {
      return types;
    }

    // Legacy `process.binding('util')` for Node.js < 10.
    return freeProcess && freeProcess.binding && freeProcess.binding('util');
  } catch (e) {}
}());

module.exports = nodeUtil;

},{"./_freeGlobal":51}],82:[function(require,module,exports){
/** Used for built-in method references. */
var objectProto = Object.prototype;

/**
 * Used to resolve the
 * [`toStringTag`](http://ecma-international.org/ecma-262/7.0/#sec-object.prototype.tostring)
 * of values.
 */
var nativeObjectToString = objectProto.toString;

/**
 * Converts `value` to a string using `Object.prototype.toString`.
 *
 * @private
 * @param {*} value The value to convert.
 * @returns {string} Returns the converted string.
 */
function objectToString(value) {
  return nativeObjectToString.call(value);
}

module.exports = objectToString;

},{}],83:[function(require,module,exports){
/**
 * Creates a unary function that invokes `func` with its argument transformed.
 *
 * @private
 * @param {Function} func The function to wrap.
 * @param {Function} transform The argument transform.
 * @returns {Function} Returns the new function.
 */
function overArg(func, transform) {
  return function(arg) {
    return func(transform(arg));
  };
}

module.exports = overArg;

},{}],84:[function(require,module,exports){
var freeGlobal = require('./_freeGlobal');

/** Detect free variable `self`. */
var freeSelf = typeof self == 'object' && self && self.Object === Object && self;

/** Used as a reference to the global object. */
var root = freeGlobal || freeSelf || Function('return this')();

module.exports = root;

},{"./_freeGlobal":51}],85:[function(require,module,exports){
/** Used to stand-in for `undefined` hash values. */
var HASH_UNDEFINED = '__lodash_hash_undefined__';

/**
 * Adds `value` to the array cache.
 *
 * @private
 * @name add
 * @memberOf SetCache
 * @alias push
 * @param {*} value The value to cache.
 * @returns {Object} Returns the cache instance.
 */
function setCacheAdd(value) {
  this.__data__.set(value, HASH_UNDEFINED);
  return this;
}

module.exports = setCacheAdd;

},{}],86:[function(require,module,exports){
/**
 * Checks if `value` is in the array cache.
 *
 * @private
 * @name has
 * @memberOf SetCache
 * @param {*} value The value to search for.
 * @returns {number} Returns `true` if `value` is found, else `false`.
 */
function setCacheHas(value) {
  return this.__data__.has(value);
}

module.exports = setCacheHas;

},{}],87:[function(require,module,exports){
/**
 * Converts `set` to an array of its values.
 *
 * @private
 * @param {Object} set The set to convert.
 * @returns {Array} Returns the values.
 */
function setToArray(set) {
  var index = -1,
      result = Array(set.size);

  set.forEach(function(value) {
    result[++index] = value;
  });
  return result;
}

module.exports = setToArray;

},{}],88:[function(require,module,exports){
var ListCache = require('./_ListCache');

/**
 * Removes all key-value entries from the stack.
 *
 * @private
 * @name clear
 * @memberOf Stack
 */
function stackClear() {
  this.__data__ = new ListCache;
  this.size = 0;
}

module.exports = stackClear;

},{"./_ListCache":21}],89:[function(require,module,exports){
/**
 * Removes `key` and its value from the stack.
 *
 * @private
 * @name delete
 * @memberOf Stack
 * @param {string} key The key of the value to remove.
 * @returns {boolean} Returns `true` if the entry was removed, else `false`.
 */
function stackDelete(key) {
  var data = this.__data__,
      result = data['delete'](key);

  this.size = data.size;
  return result;
}

module.exports = stackDelete;

},{}],90:[function(require,module,exports){
/**
 * Gets the stack value for `key`.
 *
 * @private
 * @name get
 * @memberOf Stack
 * @param {string} key The key of the value to get.
 * @returns {*} Returns the entry value.
 */
function stackGet(key) {
  return this.__data__.get(key);
}

module.exports = stackGet;

},{}],91:[function(require,module,exports){
/**
 * Checks if a stack value for `key` exists.
 *
 * @private
 * @name has
 * @memberOf Stack
 * @param {string} key The key of the entry to check.
 * @returns {boolean} Returns `true` if an entry for `key` exists, else `false`.
 */
function stackHas(key) {
  return this.__data__.has(key);
}

module.exports = stackHas;

},{}],92:[function(require,module,exports){
var ListCache = require('./_ListCache'),
    Map = require('./_Map'),
    MapCache = require('./_MapCache');

/** Used as the size to enable large array optimizations. */
var LARGE_ARRAY_SIZE = 200;

/**
 * Sets the stack `key` to `value`.
 *
 * @private
 * @name set
 * @memberOf Stack
 * @param {string} key The key of the value to set.
 * @param {*} value The value to set.
 * @returns {Object} Returns the stack cache instance.
 */
function stackSet(key, value) {
  var data = this.__data__;
  if (data instanceof ListCache) {
    var pairs = data.__data__;
    if (!Map || (pairs.length < LARGE_ARRAY_SIZE - 1)) {
      pairs.push([key, value]);
      this.size = ++data.size;
      return this;
    }
    data = this.__data__ = new MapCache(pairs);
  }
  data.set(key, value);
  this.size = data.size;
  return this;
}

module.exports = stackSet;

},{"./_ListCache":21,"./_Map":22,"./_MapCache":23}],93:[function(require,module,exports){
/** Used for built-in method references. */
var funcProto = Function.prototype;

/** Used to resolve the decompiled source of functions. */
var funcToString = funcProto.toString;

/**
 * Converts `func` to its source code.
 *
 * @private
 * @param {Function} func The function to convert.
 * @returns {string} Returns the source code.
 */
function toSource(func) {
  if (func != null) {
    try {
      return funcToString.call(func);
    } catch (e) {}
    try {
      return (func + '');
    } catch (e) {}
  }
  return '';
}

module.exports = toSource;

},{}],94:[function(require,module,exports){
/**
 * Performs a
 * [`SameValueZero`](http://ecma-international.org/ecma-262/7.0/#sec-samevaluezero)
 * comparison between two values to determine if they are equivalent.
 *
 * @static
 * @memberOf _
 * @since 4.0.0
 * @category Lang
 * @param {*} value The value to compare.
 * @param {*} other The other value to compare.
 * @returns {boolean} Returns `true` if the values are equivalent, else `false`.
 * @example
 *
 * var object = { 'a': 1 };
 * var other = { 'a': 1 };
 *
 * _.eq(object, object);
 * // => true
 *
 * _.eq(object, other);
 * // => false
 *
 * _.eq('a', 'a');
 * // => true
 *
 * _.eq('a', Object('a'));
 * // => false
 *
 * _.eq(NaN, NaN);
 * // => true
 */
function eq(value, other) {
  return value === other || (value !== value && other !== other);
}

module.exports = eq;

},{}],95:[function(require,module,exports){
var baseIsArguments = require('./_baseIsArguments'),
    isObjectLike = require('./isObjectLike');

/** Used for built-in method references. */
var objectProto = Object.prototype;

/** Used to check objects for own properties. */
var hasOwnProperty = objectProto.hasOwnProperty;

/** Built-in value references. */
var propertyIsEnumerable = objectProto.propertyIsEnumerable;

/**
 * Checks if `value` is likely an `arguments` object.
 *
 * @static
 * @memberOf _
 * @since 0.1.0
 * @category Lang
 * @param {*} value The value to check.
 * @returns {boolean} Returns `true` if `value` is an `arguments` object,
 *  else `false`.
 * @example
 *
 * _.isArguments(function() { return arguments; }());
 * // => true
 *
 * _.isArguments([1, 2, 3]);
 * // => false
 */
var isArguments = baseIsArguments(function() { return arguments; }()) ? baseIsArguments : function(value) {
  return isObjectLike(value) && hasOwnProperty.call(value, 'callee') &&
    !propertyIsEnumerable.call(value, 'callee');
};

module.exports = isArguments;

},{"./_baseIsArguments":38,"./isObjectLike":106}],96:[function(require,module,exports){
/**
 * Checks if `value` is classified as an `Array` object.
 *
 * @static
 * @memberOf _
 * @since 0.1.0
 * @category Lang
 * @param {*} value The value to check.
 * @returns {boolean} Returns `true` if `value` is an array, else `false`.
 * @example
 *
 * _.isArray([1, 2, 3]);
 * // => true
 *
 * _.isArray(document.body.children);
 * // => false
 *
 * _.isArray('abc');
 * // => false
 *
 * _.isArray(_.noop);
 * // => false
 */
var isArray = Array.isArray;

module.exports = isArray;

},{}],97:[function(require,module,exports){
var isFunction = require('./isFunction'),
    isLength = require('./isLength');

/**
 * Checks if `value` is array-like. A value is considered array-like if it's
 * not a function and has a `value.length` that's an integer greater than or
 * equal to `0` and less than or equal to `Number.MAX_SAFE_INTEGER`.
 *
 * @static
 * @memberOf _
 * @since 4.0.0
 * @category Lang
 * @param {*} value The value to check.
 * @returns {boolean} Returns `true` if `value` is array-like, else `false`.
 * @example
 *
 * _.isArrayLike([1, 2, 3]);
 * // => true
 *
 * _.isArrayLike(document.body.children);
 * // => true
 *
 * _.isArrayLike('abc');
 * // => true
 *
 * _.isArrayLike(_.noop);
 * // => false
 */
function isArrayLike(value) {
  return value != null && isLength(value.length) && !isFunction(value);
}

module.exports = isArrayLike;

},{"./isFunction":101,"./isLength":102}],98:[function(require,module,exports){
var baseGetTag = require('./_baseGetTag'),
    isObjectLike = require('./isObjectLike');

/** `Object#toString` result references. */
var boolTag = '[object Boolean]';

/**
 * Checks if `value` is classified as a boolean primitive or object.
 *
 * @static
 * @memberOf _
 * @since 0.1.0
 * @category Lang
 * @param {*} value The value to check.
 * @returns {boolean} Returns `true` if `value` is a boolean, else `false`.
 * @example
 *
 * _.isBoolean(false);
 * // => true
 *
 * _.isBoolean(null);
 * // => false
 */
function isBoolean(value) {
  return value === true || value === false ||
    (isObjectLike(value) && baseGetTag(value) == boolTag);
}

module.exports = isBoolean;

},{"./_baseGetTag":37,"./isObjectLike":106}],99:[function(require,module,exports){
var root = require('./_root'),
    stubFalse = require('./stubFalse');

/** Detect free variable `exports`. */
var freeExports = typeof exports == 'object' && exports && !exports.nodeType && exports;

/** Detect free variable `module`. */
var freeModule = freeExports && typeof module == 'object' && module && !module.nodeType && module;

/** Detect the popular CommonJS extension `module.exports`. */
var moduleExports = freeModule && freeModule.exports === freeExports;

/** Built-in value references. */
var Buffer = moduleExports ? root.Buffer : undefined;

/* Built-in method references for those with the same name as other `lodash` methods. */
var nativeIsBuffer = Buffer ? Buffer.isBuffer : undefined;

/**
 * Checks if `value` is a buffer.
 *
 * @static
 * @memberOf _
 * @since 4.3.0
 * @category Lang
 * @param {*} value The value to check.
 * @returns {boolean} Returns `true` if `value` is a buffer, else `false`.
 * @example
 *
 * _.isBuffer(new Buffer(2));
 * // => true
 *
 * _.isBuffer(new Uint8Array(2));
 * // => false
 */
var isBuffer = nativeIsBuffer || stubFalse;

module.exports = isBuffer;

},{"./_root":84,"./stubFalse":112}],100:[function(require,module,exports){
var baseIsEqual = require('./_baseIsEqual');

/**
 * Performs a deep comparison between two values to determine if they are
 * equivalent.
 *
 * **Note:** This method supports comparing arrays, array buffers, booleans,
 * date objects, error objects, maps, numbers, `Object` objects, regexes,
 * sets, strings, symbols, and typed arrays. `Object` objects are compared
 * by their own, not inherited, enumerable properties. Functions and DOM
 * nodes are compared by strict equality, i.e. `===`.
 *
 * @static
 * @memberOf _
 * @since 0.1.0
 * @category Lang
 * @param {*} value The value to compare.
 * @param {*} other The other value to compare.
 * @returns {boolean} Returns `true` if the values are equivalent, else `false`.
 * @example
 *
 * var object = { 'a': 1 };
 * var other = { 'a': 1 };
 *
 * _.isEqual(object, other);
 * // => true
 *
 * object === other;
 * // => false
 */
function isEqual(value, other) {
  return baseIsEqual(value, other);
}

module.exports = isEqual;

},{"./_baseIsEqual":39}],101:[function(require,module,exports){
var baseGetTag = require('./_baseGetTag'),
    isObject = require('./isObject');

/** `Object#toString` result references. */
var asyncTag = '[object AsyncFunction]',
    funcTag = '[object Function]',
    genTag = '[object GeneratorFunction]',
    proxyTag = '[object Proxy]';

/**
 * Checks if `value` is classified as a `Function` object.
 *
 * @static
 * @memberOf _
 * @since 0.1.0
 * @category Lang
 * @param {*} value The value to check.
 * @returns {boolean} Returns `true` if `value` is a function, else `false`.
 * @example
 *
 * _.isFunction(_);
 * // => true
 *
 * _.isFunction(/abc/);
 * // => false
 */
function isFunction(value) {
  if (!isObject(value)) {
    return false;
  }
  // The use of `Object#toString` avoids issues with the `typeof` operator
  // in Safari 9 which returns 'object' for typed arrays and other constructors.
  var tag = baseGetTag(value);
  return tag == funcTag || tag == genTag || tag == asyncTag || tag == proxyTag;
}

module.exports = isFunction;

},{"./_baseGetTag":37,"./isObject":105}],102:[function(require,module,exports){
/** Used as references for various `Number` constants. */
var MAX_SAFE_INTEGER = 9007199254740991;

/**
 * Checks if `value` is a valid array-like length.
 *
 * **Note:** This method is loosely based on
 * [`ToLength`](http://ecma-international.org/ecma-262/7.0/#sec-tolength).
 *
 * @static
 * @memberOf _
 * @since 4.0.0
 * @category Lang
 * @param {*} value The value to check.
 * @returns {boolean} Returns `true` if `value` is a valid length, else `false`.
 * @example
 *
 * _.isLength(3);
 * // => true
 *
 * _.isLength(Number.MIN_VALUE);
 * // => false
 *
 * _.isLength(Infinity);
 * // => false
 *
 * _.isLength('3');
 * // => false
 */
function isLength(value) {
  return typeof value == 'number' &&
    value > -1 && value % 1 == 0 && value <= MAX_SAFE_INTEGER;
}

module.exports = isLength;

},{}],103:[function(require,module,exports){
/**
 * Checks if `value` is `null`.
 *
 * @static
 * @memberOf _
 * @since 0.1.0
 * @category Lang
 * @param {*} value The value to check.
 * @returns {boolean} Returns `true` if `value` is `null`, else `false`.
 * @example
 *
 * _.isNull(null);
 * // => true
 *
 * _.isNull(void 0);
 * // => false
 */
function isNull(value) {
  return value === null;
}

module.exports = isNull;

},{}],104:[function(require,module,exports){
var baseGetTag = require('./_baseGetTag'),
    isObjectLike = require('./isObjectLike');

/** `Object#toString` result references. */
var numberTag = '[object Number]';

/**
 * Checks if `value` is classified as a `Number` primitive or object.
 *
 * **Note:** To exclude `Infinity`, `-Infinity`, and `NaN`, which are
 * classified as numbers, use the `_.isFinite` method.
 *
 * @static
 * @memberOf _
 * @since 0.1.0
 * @category Lang
 * @param {*} value The value to check.
 * @returns {boolean} Returns `true` if `value` is a number, else `false`.
 * @example
 *
 * _.isNumber(3);
 * // => true
 *
 * _.isNumber(Number.MIN_VALUE);
 * // => true
 *
 * _.isNumber(Infinity);
 * // => true
 *
 * _.isNumber('3');
 * // => false
 */
function isNumber(value) {
  return typeof value == 'number' ||
    (isObjectLike(value) && baseGetTag(value) == numberTag);
}

module.exports = isNumber;

},{"./_baseGetTag":37,"./isObjectLike":106}],105:[function(require,module,exports){
/**
 * Checks if `value` is the
 * [language type](http://www.ecma-international.org/ecma-262/7.0/#sec-ecmascript-language-types)
 * of `Object`. (e.g. arrays, functions, objects, regexes, `new Number(0)`, and `new String('')`)
 *
 * @static
 * @memberOf _
 * @since 0.1.0
 * @category Lang
 * @param {*} value The value to check.
 * @returns {boolean} Returns `true` if `value` is an object, else `false`.
 * @example
 *
 * _.isObject({});
 * // => true
 *
 * _.isObject([1, 2, 3]);
 * // => true
 *
 * _.isObject(_.noop);
 * // => true
 *
 * _.isObject(null);
 * // => false
 */
function isObject(value) {
  var type = typeof value;
  return value != null && (type == 'object' || type == 'function');
}

module.exports = isObject;

},{}],106:[function(require,module,exports){
/**
 * Checks if `value` is object-like. A value is object-like if it's not `null`
 * and has a `typeof` result of "object".
 *
 * @static
 * @memberOf _
 * @since 4.0.0
 * @category Lang
 * @param {*} value The value to check.
 * @returns {boolean} Returns `true` if `value` is object-like, else `false`.
 * @example
 *
 * _.isObjectLike({});
 * // => true
 *
 * _.isObjectLike([1, 2, 3]);
 * // => true
 *
 * _.isObjectLike(_.noop);
 * // => false
 *
 * _.isObjectLike(null);
 * // => false
 */
function isObjectLike(value) {
  return value != null && typeof value == 'object';
}

module.exports = isObjectLike;

},{}],107:[function(require,module,exports){
var baseGetTag = require('./_baseGetTag'),
    isArray = require('./isArray'),
    isObjectLike = require('./isObjectLike');

/** `Object#toString` result references. */
var stringTag = '[object String]';

/**
 * Checks if `value` is classified as a `String` primitive or object.
 *
 * @static
 * @since 0.1.0
 * @memberOf _
 * @category Lang
 * @param {*} value The value to check.
 * @returns {boolean} Returns `true` if `value` is a string, else `false`.
 * @example
 *
 * _.isString('abc');
 * // => true
 *
 * _.isString(1);
 * // => false
 */
function isString(value) {
  return typeof value == 'string' ||
    (!isArray(value) && isObjectLike(value) && baseGetTag(value) == stringTag);
}

module.exports = isString;

},{"./_baseGetTag":37,"./isArray":96,"./isObjectLike":106}],108:[function(require,module,exports){
var baseIsTypedArray = require('./_baseIsTypedArray'),
    baseUnary = require('./_baseUnary'),
    nodeUtil = require('./_nodeUtil');

/* Node.js helper references. */
var nodeIsTypedArray = nodeUtil && nodeUtil.isTypedArray;

/**
 * Checks if `value` is classified as a typed array.
 *
 * @static
 * @memberOf _
 * @since 3.0.0
 * @category Lang
 * @param {*} value The value to check.
 * @returns {boolean} Returns `true` if `value` is a typed array, else `false`.
 * @example
 *
 * _.isTypedArray(new Uint8Array);
 * // => true
 *
 * _.isTypedArray([]);
 * // => false
 */
var isTypedArray = nodeIsTypedArray ? baseUnary(nodeIsTypedArray) : baseIsTypedArray;

module.exports = isTypedArray;

},{"./_baseIsTypedArray":42,"./_baseUnary":45,"./_nodeUtil":81}],109:[function(require,module,exports){
var arrayLikeKeys = require('./_arrayLikeKeys'),
    baseKeys = require('./_baseKeys'),
    isArrayLike = require('./isArrayLike');

/**
 * Creates an array of the own enumerable property names of `object`.
 *
 * **Note:** Non-object values are coerced to objects. See the
 * [ES spec](http://ecma-international.org/ecma-262/7.0/#sec-object.keys)
 * for more details.
 *
 * @static
 * @since 0.1.0
 * @memberOf _
 * @category Object
 * @param {Object} object The object to query.
 * @returns {Array} Returns the array of property names.
 * @example
 *
 * function Foo() {
 *   this.a = 1;
 *   this.b = 2;
 * }
 *
 * Foo.prototype.c = 3;
 *
 * _.keys(new Foo);
 * // => ['a', 'b'] (iteration order is not guaranteed)
 *
 * _.keys('hi');
 * // => ['0', '1']
 */
function keys(object) {
  return isArrayLike(object) ? arrayLikeKeys(object) : baseKeys(object);
}

module.exports = keys;

},{"./_arrayLikeKeys":32,"./_baseKeys":43,"./isArrayLike":97}],110:[function(require,module,exports){
/** Error message constants. */
var FUNC_ERROR_TEXT = 'Expected a function';

/**
 * Creates a function that negates the result of the predicate `func`. The
 * `func` predicate is invoked with the `this` binding and arguments of the
 * created function.
 *
 * @static
 * @memberOf _
 * @since 3.0.0
 * @category Function
 * @param {Function} predicate The predicate to negate.
 * @returns {Function} Returns the new negated function.
 * @example
 *
 * function isEven(n) {
 *   return n % 2 == 0;
 * }
 *
 * _.filter([1, 2, 3, 4, 5, 6], _.negate(isEven));
 * // => [1, 3, 5]
 */
function negate(predicate) {
  if (typeof predicate != 'function') {
    throw new TypeError(FUNC_ERROR_TEXT);
  }
  return function() {
    var args = arguments;
    switch (args.length) {
      case 0: return !predicate.call(this);
      case 1: return !predicate.call(this, args[0]);
      case 2: return !predicate.call(this, args[0], args[1]);
      case 3: return !predicate.call(this, args[0], args[1], args[2]);
    }
    return !predicate.apply(this, args);
  };
}

module.exports = negate;

},{}],111:[function(require,module,exports){
/**
 * This method returns a new empty array.
 *
 * @static
 * @memberOf _
 * @since 4.13.0
 * @category Util
 * @returns {Array} Returns the new empty array.
 * @example
 *
 * var arrays = _.times(2, _.stubArray);
 *
 * console.log(arrays);
 * // => [[], []]
 *
 * console.log(arrays[0] === arrays[1]);
 * // => false
 */
function stubArray() {
  return [];
}

module.exports = stubArray;

},{}],112:[function(require,module,exports){
/**
 * This method returns `false`.
 *
 * @static
 * @memberOf _
 * @since 4.13.0
 * @category Util
 * @returns {boolean} Returns `false`.
 * @example
 *
 * _.times(2, _.stubFalse);
 * // => [false, false]
 */
function stubFalse() {
  return false;
}

module.exports = stubFalse;

},{}]},{},[8])(8)
});

//# sourceMappingURL=data:application/json;charset=utf-8;base64,eyJ2ZXJzaW9uIjozLCJzb3VyY2VzIjpbIm5vZGVfbW9kdWxlcy9icm93c2VyLXBhY2svX3ByZWx1ZGUuanMiLCJkaXN0L2xpYi9BcnJheVNsaWNlLmpzIiwiZGlzdC9saWIvS2V5VmFsdWVQYWlyLmpzIiwiZGlzdC9saWIvTmFtZXNwYWNlLmpzIiwiZGlzdC9saWIvT2JqZWN0U2xpY2UuanMiLCJkaXN0L2xpYi9lbGVtZW50cy5qcyIsImRpc3QvbGliL2VsZW1lbnRzL0xpbmtFbGVtZW50LmpzIiwiZGlzdC9saWIvZWxlbWVudHMvUmVmRWxlbWVudC5qcyIsImRpc3QvbGliL21pbmltLmpzIiwiZGlzdC9saWIvcHJpbWl0aXZlcy9BcnJheUVsZW1lbnQuanMiLCJkaXN0L2xpYi9wcmltaXRpdmVzL0Jvb2xlYW5FbGVtZW50LmpzIiwiZGlzdC9saWIvcHJpbWl0aXZlcy9FbGVtZW50LmpzIiwiZGlzdC9saWIvcHJpbWl0aXZlcy9NZW1iZXJFbGVtZW50LmpzIiwiZGlzdC9saWIvcHJpbWl0aXZlcy9OdWxsRWxlbWVudC5qcyIsImRpc3QvbGliL3ByaW1pdGl2ZXMvTnVtYmVyRWxlbWVudC5qcyIsImRpc3QvbGliL3ByaW1pdGl2ZXMvT2JqZWN0RWxlbWVudC5qcyIsImRpc3QvbGliL3ByaW1pdGl2ZXMvU3RyaW5nRWxlbWVudC5qcyIsImRpc3QvbGliL3NlcmlhbGlzZXJzL0pTT04wNlNlcmlhbGlzZXIuanMiLCJkaXN0L2xpYi9zZXJpYWxpc2Vycy9KU09OU2VyaWFsaXNlci5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvX0RhdGFWaWV3LmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fSGFzaC5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvX0xpc3RDYWNoZS5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvX01hcC5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvX01hcENhY2hlLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fUHJvbWlzZS5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvX1NldC5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvX1NldENhY2hlLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fU3RhY2suanMiLCJub2RlX21vZHVsZXMvbG9kYXNoL19TeW1ib2wuanMiLCJub2RlX21vZHVsZXMvbG9kYXNoL19VaW50OEFycmF5LmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fV2Vha01hcC5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvX2FycmF5RmlsdGVyLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fYXJyYXlMaWtlS2V5cy5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvX2FycmF5UHVzaC5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvX2FycmF5U29tZS5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvX2Fzc29jSW5kZXhPZi5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvX2Jhc2VHZXRBbGxLZXlzLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fYmFzZUdldFRhZy5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvX2Jhc2VJc0FyZ3VtZW50cy5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvX2Jhc2VJc0VxdWFsLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fYmFzZUlzRXF1YWxEZWVwLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fYmFzZUlzTmF0aXZlLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fYmFzZUlzVHlwZWRBcnJheS5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvX2Jhc2VLZXlzLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fYmFzZVRpbWVzLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fYmFzZVVuYXJ5LmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fY2FjaGVIYXMuanMiLCJub2RlX21vZHVsZXMvbG9kYXNoL19jb3JlSnNEYXRhLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fZXF1YWxBcnJheXMuanMiLCJub2RlX21vZHVsZXMvbG9kYXNoL19lcXVhbEJ5VGFnLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fZXF1YWxPYmplY3RzLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fZnJlZUdsb2JhbC5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvX2dldEFsbEtleXMuanMiLCJub2RlX21vZHVsZXMvbG9kYXNoL19nZXRNYXBEYXRhLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fZ2V0TmF0aXZlLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fZ2V0UmF3VGFnLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fZ2V0U3ltYm9scy5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvX2dldFRhZy5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvX2dldFZhbHVlLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9faGFzaENsZWFyLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9faGFzaERlbGV0ZS5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvX2hhc2hHZXQuanMiLCJub2RlX21vZHVsZXMvbG9kYXNoL19oYXNoSGFzLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9faGFzaFNldC5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvX2lzSW5kZXguanMiLCJub2RlX21vZHVsZXMvbG9kYXNoL19pc0tleWFibGUuanMiLCJub2RlX21vZHVsZXMvbG9kYXNoL19pc01hc2tlZC5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvX2lzUHJvdG90eXBlLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fbGlzdENhY2hlQ2xlYXIuanMiLCJub2RlX21vZHVsZXMvbG9kYXNoL19saXN0Q2FjaGVEZWxldGUuanMiLCJub2RlX21vZHVsZXMvbG9kYXNoL19saXN0Q2FjaGVHZXQuanMiLCJub2RlX21vZHVsZXMvbG9kYXNoL19saXN0Q2FjaGVIYXMuanMiLCJub2RlX21vZHVsZXMvbG9kYXNoL19saXN0Q2FjaGVTZXQuanMiLCJub2RlX21vZHVsZXMvbG9kYXNoL19tYXBDYWNoZUNsZWFyLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fbWFwQ2FjaGVEZWxldGUuanMiLCJub2RlX21vZHVsZXMvbG9kYXNoL19tYXBDYWNoZUdldC5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvX21hcENhY2hlSGFzLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fbWFwQ2FjaGVTZXQuanMiLCJub2RlX21vZHVsZXMvbG9kYXNoL19tYXBUb0FycmF5LmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fbmF0aXZlQ3JlYXRlLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fbmF0aXZlS2V5cy5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvX25vZGVVdGlsLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fb2JqZWN0VG9TdHJpbmcuanMiLCJub2RlX21vZHVsZXMvbG9kYXNoL19vdmVyQXJnLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fcm9vdC5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvX3NldENhY2hlQWRkLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fc2V0Q2FjaGVIYXMuanMiLCJub2RlX21vZHVsZXMvbG9kYXNoL19zZXRUb0FycmF5LmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fc3RhY2tDbGVhci5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvX3N0YWNrRGVsZXRlLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fc3RhY2tHZXQuanMiLCJub2RlX21vZHVsZXMvbG9kYXNoL19zdGFja0hhcy5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvX3N0YWNrU2V0LmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9fdG9Tb3VyY2UuanMiLCJub2RlX21vZHVsZXMvbG9kYXNoL2VxLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9pc0FyZ3VtZW50cy5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvaXNBcnJheS5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvaXNBcnJheUxpa2UuanMiLCJub2RlX21vZHVsZXMvbG9kYXNoL2lzQm9vbGVhbi5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvaXNCdWZmZXIuanMiLCJub2RlX21vZHVsZXMvbG9kYXNoL2lzRXF1YWwuanMiLCJub2RlX21vZHVsZXMvbG9kYXNoL2lzRnVuY3Rpb24uanMiLCJub2RlX21vZHVsZXMvbG9kYXNoL2lzTGVuZ3RoLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9pc051bGwuanMiLCJub2RlX21vZHVsZXMvbG9kYXNoL2lzTnVtYmVyLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9pc09iamVjdC5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvaXNPYmplY3RMaWtlLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9pc1N0cmluZy5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvaXNUeXBlZEFycmF5LmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9rZXlzLmpzIiwibm9kZV9tb2R1bGVzL2xvZGFzaC9uZWdhdGUuanMiLCJub2RlX21vZHVsZXMvbG9kYXNoL3N0dWJBcnJheS5qcyIsIm5vZGVfbW9kdWxlcy9sb2Rhc2gvc3R1YkZhbHNlLmpzIl0sIm5hbWVzIjpbXSwibWFwcGluZ3MiOiJBQUFBO0FDQUE7O0FBRUEsSUFBSSxlQUFlLFlBQVk7QUFBRSxXQUFTLGdCQUFULENBQTBCLE1BQTFCLEVBQWtDLEtBQWxDLEVBQXlDO0FBQUUsU0FBSyxJQUFJLElBQUksQ0FBYixFQUFnQixJQUFJLE1BQU0sTUFBMUIsRUFBa0MsR0FBbEMsRUFBdUM7QUFBRSxVQUFJLGFBQWEsTUFBTSxDQUFOLENBQWpCLENBQTJCLFdBQVcsVUFBWCxHQUF3QixXQUFXLFVBQVgsSUFBeUIsS0FBakQsQ0FBd0QsV0FBVyxZQUFYLEdBQTBCLElBQTFCLENBQWdDLElBQUksV0FBVyxVQUFmLEVBQTJCLFdBQVcsUUFBWCxHQUFzQixJQUF0QixDQUE0QixPQUFPLGNBQVAsQ0FBc0IsTUFBdEIsRUFBOEIsV0FBVyxHQUF6QyxFQUE4QyxVQUE5QztBQUE0RDtBQUFFLEdBQUMsT0FBTyxVQUFVLFdBQVYsRUFBdUIsVUFBdkIsRUFBbUMsV0FBbkMsRUFBZ0Q7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QUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUNoQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUNQQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQ1BBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQzNCQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUMzQkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDTkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDTkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUNQQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQ3pCQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQ2pEQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDcEJBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUN2QkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDckJBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUNwQkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUM1QkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDbEJBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDNUJBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUNuRkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQy9DQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUM1REE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDOUJBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUNwQkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQ2RBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDYkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDTkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQ25GQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQ2hIQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7OztBQ3pGQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOzs7O0FDSkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUNoQkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDbEJBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUNqQkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUM5Q0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDOUJBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDMURBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDYkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDZkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQ2pCQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUM5QkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQ3ZCQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDdkJBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDekJBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQ2ZBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUNwQkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDbEJBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDYkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQ25DQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQ25CQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQ2hCQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDMUJBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQ3JCQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUNsQkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUNoQkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUNoQkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUN0QkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDbEJBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQ05BO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQ05BO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQzlCQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQ3RCQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUNmQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUNUQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQ25CQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDZEE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDbEJBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQ2ZBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQ2xCQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDZEE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQ2RBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDbENBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUMxQkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUNyQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDcENBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUMxQkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDakNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUM3QkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQ3RDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDbkNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDckNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUNuQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUN0QkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQ3RDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQy9CQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDN0JBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQzlCQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUMzQkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUNyQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUN4Q0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQ3ZCQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQSIsImZpbGUiOiJnZW5lcmF0ZWQuanMiLCJzb3VyY2VSb290IjoiIiwic291cmNlc0NvbnRlbnQiOlsiKGZ1bmN0aW9uKCl7ZnVuY3Rpb24gcihlLG4sdCl7ZnVuY3Rpb24gbyhpLGYpe2lmKCFuW2ldKXtpZighZVtpXSl7dmFyIGM9XCJmdW5jdGlvblwiPT10eXBlb2YgcmVxdWlyZSYmcmVxdWlyZTtpZighZiYmYylyZXR1cm4gYyhpLCEwKTtpZih1KXJldHVybiB1KGksITApO3ZhciBhPW5ldyBFcnJvcihcIkNhbm5vdCBmaW5kIG1vZHVsZSAnXCIraStcIidcIik7dGhyb3cgYS5jb2RlPVwiTU9EVUxFX05PVF9GT1VORFwiLGF9dmFyIHA9bltpXT17ZXhwb3J0czp7fX07ZVtpXVswXS5jYWxsKHAuZXhwb3J0cyxmdW5jdGlvbihyKXt2YXIgbj1lW2ldWzFdW3JdO3JldHVybiBvKG58fHIpfSxwLHAuZXhwb3J0cyxyLGUsbix0KX1yZXR1cm4gbltpXS5leHBvcnRzfWZvcih2YXIgdT1cImZ1bmN0aW9uXCI9PXR5cGVvZiByZXF1aXJlJiZyZXF1aXJlLGk9MDtpPHQubGVuZ3RoO2krKylvKHRbaV0pO3JldHVybiBvfXJldHVybiByfSkoKSIsIid1c2Ugc3RyaWN0JztcblxudmFyIF9jcmVhdGVDbGFzcyA9IGZ1bmN0aW9uICgpIHsgZnVuY3Rpb24gZGVmaW5lUHJvcGVydGllcyh0YXJnZXQsIHByb3BzKSB7IGZvciAodmFyIGkgPSAwOyBpIDwgcHJvcHMubGVuZ3RoOyBpKyspIHsgdmFyIGRlc2NyaXB0b3IgPSBwcm9wc1tpXTsgZGVzY3JpcHRvci5lbnVtZXJhYmxlID0gZGVzY3JpcHRvci5lbnVtZXJhYmxlIHx8IGZhbHNlOyBkZXNjcmlwdG9yLmNvbmZpZ3VyYWJsZSA9IHRydWU7IGlmIChcInZhbHVlXCIgaW4gZGVzY3JpcHRvcikgZGVzY3JpcHRvci53cml0YWJsZSA9IHRydWU7IE9iamVjdC5kZWZpbmVQcm9wZXJ0eSh0YXJnZXQsIGRlc2NyaXB0b3Iua2V5LCBkZXNjcmlwdG9yKTsgfSB9IHJldHVybiBmdW5jdGlvbiAoQ29uc3RydWN0b3IsIHByb3RvUHJvcHMsIHN0YXRpY1Byb3BzKSB7IGlmIChwcm90b1Byb3BzKSBkZWZpbmVQcm9wZXJ0aWVzKENvbnN0cnVjdG9yLnByb3RvdHlwZSwgcHJvdG9Qcm9wcyk7IGlmIChzdGF0aWNQcm9wcykgZGVmaW5lUHJvcGVydGllcyhDb25zdHJ1Y3Rvciwgc3RhdGljUHJvcHMpOyByZXR1cm4gQ29uc3RydWN0b3I7IH07IH0oKTtcblxuZnVuY3Rpb24gX2NsYXNzQ2FsbENoZWNrKGluc3RhbmNlLCBDb25zdHJ1Y3RvcikgeyBpZiAoIShpbnN0YW5jZSBpbnN0YW5jZW9mIENvbnN0cnVjdG9yKSkgeyB0aHJvdyBuZXcgVHlwZUVycm9yKFwiQ2Fubm90IGNhbGwgYSBjbGFzcyBhcyBhIGZ1bmN0aW9uXCIpOyB9IH1cblxudmFyIG5lZ2F0ZSA9IHJlcXVpcmUoJ2xvZGFzaC9uZWdhdGUnKTtcblxuLy8gQ29lcmNlcyBhbiBhIHBhcmFtZXRlciBpbnRvIGEgY2FsbGJhY2sgZm9yIG1hdGNoaW5nIGVsZW1lbnRzLlxuLy8gVGhpcyBhY2NlcHRzIGFuIGVsZW1lbnQgbmFtZSwgYW4gZWxlbWVudCB0eXBlIGFuZCByZXR1cm5zIGFcbi8vIGNhbGxiYWNrIHRvIG1hdGNoIGZvciB0aG9zZSBlbGVtZW50cy5cbmZ1bmN0aW9uIGNvZXJjZUVsZW1lbnRNYXRjaGluZ0NhbGxiYWNrKHZhbHVlKSB7XG4gIC8vIEVsZW1lbnQgTmFtZVxuICBpZiAodHlwZW9mIHZhbHVlID09PSAnc3RyaW5nJykge1xuICAgIHJldHVybiBmdW5jdGlvbiAoZWxlbWVudCkge1xuICAgICAgcmV0dXJuIGVsZW1lbnQuZWxlbWVudCA9PT0gdmFsdWU7XG4gICAgfTtcbiAgfVxuXG4gIC8vIEVsZW1lbnQgVHlwZVxuICBpZiAodmFsdWUuY29uc3RydWN0b3IgJiYgdmFsdWUuZXh0ZW5kKSB7XG4gICAgcmV0dXJuIGZ1bmN0aW9uIChlbGVtZW50KSB7XG4gICAgICByZXR1cm4gZWxlbWVudCBpbnN0YW5jZW9mIHZhbHVlO1xuICAgIH07XG4gIH1cblxuICByZXR1cm4gdmFsdWU7XG59XG5cbi8qKlxuICogQGNsYXNzXG4gKlxuICogQHBhcmFtIHtFbGVtZW50W119IGVsZW1lbnRzXG4gKlxuICogQHByb3BlcnR5IHtFbGVtZW50W119IGVsZW1lbnRzXG4gKi9cblxudmFyIEFycmF5U2xpY2UgPSBmdW5jdGlvbiAoKSB7XG4gIGZ1bmN0aW9uIEFycmF5U2xpY2UoZWxlbWVudHMpIHtcbiAgICBfY2xhc3NDYWxsQ2hlY2sodGhpcywgQXJyYXlTbGljZSk7XG5cbiAgICB0aGlzLmVsZW1lbnRzID0gZWxlbWVudHMgfHwgW107XG4gIH1cblxuICAvKipcbiAgICogQHJldHVybnMge0FycmF5fVxuICAgKi9cblxuXG4gIF9jcmVhdGVDbGFzcyhBcnJheVNsaWNlLCBbe1xuICAgIGtleTogJ3RvVmFsdWUnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiB0b1ZhbHVlKCkge1xuICAgICAgcmV0dXJuIHRoaXMuZWxlbWVudHMubWFwKGZ1bmN0aW9uIChlbGVtZW50KSB7XG4gICAgICAgIHJldHVybiBlbGVtZW50LnRvVmFsdWUoKTtcbiAgICAgIH0pO1xuICAgIH1cblxuICAgIC8vIEhpZ2ggT3JkZXIgRnVuY3Rpb25zXG5cbiAgICAvKipcbiAgICAgKiBAcGFyYW0gY2FsbGJhY2sgLSBGdW5jdGlvbiB0byBleGVjdXRlIGZvciBlYWNoIGVsZW1lbnRcbiAgICAgKiBAcGFyYW0gdGhpc0FyZyAtIFZhbHVlIHRvIHVzZSBhcyB0aGlzIChpLmUgdGhlIHJlZmVyZW5jZSBPYmplY3QpIHdoZW4gZXhlY3V0aW5nIGNhbGxiYWNrXG4gICAgICogQHJldHVybnMge2FycmF5fSBBIG5ldyBhcnJheSB3aXRoIGVhY2ggZWxlbWVudCBiZWluZyB0aGUgcmVzdWx0IG9mIHRoZSBjYWxsYmFjayBmdW5jdGlvblxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdtYXAnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBtYXAoY2FsbGJhY2ssIHRoaXNBcmcpIHtcbiAgICAgIHJldHVybiB0aGlzLmVsZW1lbnRzLm1hcChjYWxsYmFjaywgdGhpc0FyZyk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogTWFwcyBhbmQgdGhlbiBmbGF0dGVucyB0aGUgcmVzdWx0cy5cbiAgICAgKiBAcGFyYW0gY2FsbGJhY2sgLSBGdW5jdGlvbiB0byBleGVjdXRlIGZvciBlYWNoIGVsZW1lbnQuXG4gICAgICogQHBhcmFtIHRoaXNBcmcgLSBWYWx1ZSB0byB1c2UgYXMgdGhpcyAoaS5lIHRoZSByZWZlcmVuY2UgT2JqZWN0KSB3aGVuIGV4ZWN1dGluZyBjYWxsYmFja1xuICAgICAqIEByZXR1cm5zIHthcnJheX1cbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAnZmxhdE1hcCcsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIGZsYXRNYXAoY2FsbGJhY2ssIHRoaXNBcmcpIHtcbiAgICAgIHJldHVybiB0aGlzLm1hcChjYWxsYmFjaywgdGhpc0FyZykucmVkdWNlKGZ1bmN0aW9uIChhLCBiKSB7XG4gICAgICAgIHJldHVybiBhLmNvbmNhdChiKTtcbiAgICAgIH0sIFtdKTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBSZXR1cm5zIGFuIGFycmF5IGNvbnRhaW5pbmcgdGhlIHRydXRoeSByZXN1bHRzIG9mIGNhbGxpbmcgdGhlIGdpdmVuIHRyYW5zZm9ybWF0aW9uIHdpdGggZWFjaCBlbGVtZW50IG9mIHRoaXMgc2VxdWVuY2VcbiAgICAgKiBAcGFyYW0gdHJhbnNmb3JtIC0gQSBjbG9zdXJlIHRoYXQgYWNjZXB0cyBhbiBlbGVtZW50IG9mIHRoaXMgYXJyYXkgYXMgaXRzIGFyZ3VtZW50IGFuZCByZXR1cm5zIGFuIG9wdGlvbmFsIHZhbHVlLlxuICAgICAqIEBwYXJhbSB0aGlzQXJnIC0gVmFsdWUgdG8gdXNlIGFzIHRoaXMgKGkuZSB0aGUgcmVmZXJlbmNlIE9iamVjdCkgd2hlbiBleGVjdXRpbmcgY2FsbGJhY2tcbiAgICAgKiBAbWVtYmVyb2YgQXJyYXlTbGljZS5wcm90b3R5cGVcbiAgICAgKiBAcmV0dXJucyBBbiBhcnJheSBvZiB0aGUgbm9uLXVuZGVmaW5lZCByZXN1bHRzIG9mIGNhbGxpbmcgdHJhbnNmb3JtIHdpdGggZWFjaCBlbGVtZW50IG9mIHRoZSBhcnJheVxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdjb21wYWN0TWFwJyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gY29tcGFjdE1hcCh0cmFuc2Zvcm0sIHRoaXNBcmcpIHtcbiAgICAgIHZhciByZXN1bHRzID0gW107XG5cbiAgICAgIHRoaXMuZm9yRWFjaChmdW5jdGlvbiAoZWxlbWVudCkge1xuICAgICAgICB2YXIgcmVzdWx0ID0gdHJhbnNmb3JtLmJpbmQodGhpc0FyZykoZWxlbWVudCk7XG5cbiAgICAgICAgaWYgKHJlc3VsdCkge1xuICAgICAgICAgIHJlc3VsdHMucHVzaChyZXN1bHQpO1xuICAgICAgICB9XG4gICAgICB9KTtcblxuICAgICAgcmV0dXJuIHJlc3VsdHM7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQHBhcmFtIGNhbGxiYWNrIC0gRnVuY3Rpb24gdG8gZXhlY3V0ZSBmb3IgZWFjaCBlbGVtZW50LiBUaGlzIG1heSBiZSBhIGNhbGxiYWNrLCBhbiBlbGVtZW50IG5hbWUgb3IgYW4gZWxlbWVudCBjbGFzcy5cbiAgICAgKiBAcGFyYW0gdGhpc0FyZyAtIFZhbHVlIHRvIHVzZSBhcyB0aGlzIChpLmUgdGhlIHJlZmVyZW5jZSBPYmplY3QpIHdoZW4gZXhlY3V0aW5nIGNhbGxiYWNrXG4gICAgICogQHJldHVybnMge0FycmF5U2xpY2V9XG4gICAgICogQG1lbWJlcm9mIEFycmF5U2xpY2UucHJvdG90eXBlXG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ2ZpbHRlcicsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIGZpbHRlcihjYWxsYmFjaywgdGhpc0FyZykge1xuICAgICAgY2FsbGJhY2sgPSBjb2VyY2VFbGVtZW50TWF0Y2hpbmdDYWxsYmFjayhjYWxsYmFjayk7XG4gICAgICByZXR1cm4gbmV3IEFycmF5U2xpY2UodGhpcy5lbGVtZW50cy5maWx0ZXIoY2FsbGJhY2ssIHRoaXNBcmcpKTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBAcGFyYW0gY2FsbGJhY2sgLSBGdW5jdGlvbiB0byBleGVjdXRlIGZvciBlYWNoIGVsZW1lbnQuIFRoaXMgbWF5IGJlIGEgY2FsbGJhY2ssIGFuIGVsZW1lbnQgbmFtZSBvciBhbiBlbGVtZW50IGNsYXNzLlxuICAgICAqIEBwYXJhbSB0aGlzQXJnIC0gVmFsdWUgdG8gdXNlIGFzIHRoaXMgKGkuZSB0aGUgcmVmZXJlbmNlIE9iamVjdCkgd2hlbiBleGVjdXRpbmcgY2FsbGJhY2tcbiAgICAgKiBAcmV0dXJucyB7QXJyYXlTbGljZX1cbiAgICAgKiBAbWVtYmVyb2YgQXJyYXlTbGljZS5wcm90b3R5cGVcbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAncmVqZWN0JyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gcmVqZWN0KGNhbGxiYWNrLCB0aGlzQXJnKSB7XG4gICAgICBjYWxsYmFjayA9IGNvZXJjZUVsZW1lbnRNYXRjaGluZ0NhbGxiYWNrKGNhbGxiYWNrKTtcbiAgICAgIHJldHVybiBuZXcgQXJyYXlTbGljZSh0aGlzLmVsZW1lbnRzLmZpbHRlcihuZWdhdGUoY2FsbGJhY2spLCB0aGlzQXJnKSk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogUmV0dXJucyB0aGUgZmlyc3QgZWxlbWVudCBpbiB0aGUgYXJyYXkgdGhhdCBzYXRpc2ZpZXMgdGhlIGdpdmVuIHZhbHVlXG4gICAgICogQHBhcmFtIGNhbGxiYWNrIC0gRnVuY3Rpb24gdG8gZXhlY3V0ZSBmb3IgZWFjaCBlbGVtZW50LiBUaGlzIG1heSBiZSBhIGNhbGxiYWNrLCBhbiBlbGVtZW50IG5hbWUgb3IgYW4gZWxlbWVudCBjbGFzcy5cbiAgICAgKiBAcGFyYW0gdGhpc0FyZyAtIFZhbHVlIHRvIHVzZSBhcyB0aGlzIChpLmUgdGhlIHJlZmVyZW5jZSBPYmplY3QpIHdoZW4gZXhlY3V0aW5nIGNhbGxiYWNrXG4gICAgICogQHJldHVybnMge0VsZW1lbnR9XG4gICAgICogQG1lbWJlcm9mIEFycmF5U2xpY2UucHJvdG90eXBlXG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ2ZpbmQnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBmaW5kKGNhbGxiYWNrLCB0aGlzQXJnKSB7XG4gICAgICBjYWxsYmFjayA9IGNvZXJjZUVsZW1lbnRNYXRjaGluZ0NhbGxiYWNrKGNhbGxiYWNrKTtcbiAgICAgIHJldHVybiB0aGlzLmVsZW1lbnRzLmZpbmQoY2FsbGJhY2ssIHRoaXNBcmcpO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEBwYXJhbSBjYWxsYmFjayAtIEZ1bmN0aW9uIHRvIGV4ZWN1dGUgZm9yIGVhY2ggZWxlbWVudFxuICAgICAqIEBwYXJhbSB0aGlzQXJnIC0gVmFsdWUgdG8gdXNlIGFzIHRoaXMgKGkuZSB0aGUgcmVmZXJlbmNlIE9iamVjdCkgd2hlbiBleGVjdXRpbmcgY2FsbGJhY2tcbiAgICAgKiBAbWVtYmVyb2YgQXJyYXlTbGljZS5wcm90b3R5cGVcbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAnZm9yRWFjaCcsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIGZvckVhY2goY2FsbGJhY2ssIHRoaXNBcmcpIHtcbiAgICAgIHRoaXMuZWxlbWVudHMuZm9yRWFjaChjYWxsYmFjaywgdGhpc0FyZyk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQHBhcmFtIGNhbGxiYWNrIC0gRnVuY3Rpb24gdG8gZXhlY3V0ZSBmb3IgZWFjaCBlbGVtZW50XG4gICAgICogQHBhcmFtIGluaXRpYWxWYWx1ZVxuICAgICAqIEBtZW1iZXJvZiBBcnJheVNsaWNlLnByb3RvdHlwZVxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdyZWR1Y2UnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiByZWR1Y2UoY2FsbGJhY2ssIGluaXRpYWxWYWx1ZSkge1xuICAgICAgcmV0dXJuIHRoaXMuZWxlbWVudHMucmVkdWNlKGNhbGxiYWNrLCBpbml0aWFsVmFsdWUpO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEBwYXJhbSB2YWx1ZVxuICAgICAqIEByZXR1cm5zIHtib29sZWFufVxuICAgICAqIEBtZW1iZXJvZiBBcnJheVNsaWNlLnByb3RvdHlwZVxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdpbmNsdWRlcycsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIGluY2x1ZGVzKHZhbHVlKSB7XG4gICAgICByZXR1cm4gdGhpcy5lbGVtZW50cy5zb21lKGZ1bmN0aW9uIChlbGVtZW50KSB7XG4gICAgICAgIHJldHVybiBlbGVtZW50LmVxdWFscyh2YWx1ZSk7XG4gICAgICB9KTtcbiAgICB9XG5cbiAgICAvLyBNdXRhdGlvblxuXG4gICAgLyoqXG4gICAgICogUmVtb3ZlcyB0aGUgZmlyc3QgZWxlbWVudCBmcm9tIHRoZSBzbGljZVxuICAgICAqIEByZXR1cm5zIHtFbGVtZW50fSBUaGUgcmVtb3ZlZCBlbGVtZW50IG9yIHVuZGVmaW5lZCBpZiB0aGUgc2xpY2UgaXMgZW1wdHlcbiAgICAgKiBAbWVtYmVyb2YgQXJyYXlTbGljZS5wcm90b3R5cGVcbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAnc2hpZnQnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBzaGlmdCgpIHtcbiAgICAgIHJldHVybiB0aGlzLmVsZW1lbnRzLnNoaWZ0KCk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQWRkcyB0aGUgZ2l2ZW4gZWxlbWVudCB0byB0aGUgYmVnaW5pbmcgb2YgdGhlIHNsaWNlXG4gICAgICogQHBhcmFtZXRlciB7RWxlbWVudH0gdmFsdWVcbiAgICAgKiBAbWVtYmVyb2YgQXJyYXlTbGljZS5wcm90b3R5cGVcbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAndW5zaGlmdCcsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIHVuc2hpZnQodmFsdWUpIHtcbiAgICAgIHRoaXMuZWxlbWVudHMudW5zaGlmdCh0aGlzLnJlZnJhY3QodmFsdWUpKTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBBZGRzIHRoZSBnaXZlbiBlbGVtZW50IHRvIHRoZSBlbmQgb2YgdGhlIHNsaWNlXG4gICAgICogQHBhcmFtZXRlciB7RWxlbWVudH0gdmFsdWVcbiAgICAgKiBAbWVtYmVyb2YgQXJyYXlTbGljZS5wcm90b3R5cGVcbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAncHVzaCcsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIHB1c2godmFsdWUpIHtcbiAgICAgIHRoaXMuZWxlbWVudHMucHVzaCh0aGlzLnJlZnJhY3QodmFsdWUpKTtcbiAgICAgIHJldHVybiB0aGlzO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEBwYXJhbWV0ZXIge0VsZW1lbnR9IHZhbHVlXG4gICAgICogQG1lbWJlcm9mIEFycmF5U2xpY2UucHJvdG90eXBlXG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ2FkZCcsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIGFkZCh2YWx1ZSkge1xuICAgICAgdGhpcy5wdXNoKHZhbHVlKTtcbiAgICB9XG5cbiAgICAvLyBBY2Nlc3NvcnNcblxuICAgIC8qKlxuICAgICAqIEBwYXJhbWV0ZXIge251bWJlcn0gaW5kZXhcbiAgICAgKiBAcmV0dXJucyB7RWxlbWVudH1cbiAgICAgKiBAbWVtYmVyb2YgQXJyYXlTbGljZS5wcm90b3R5cGVcbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAnZ2V0JyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gZ2V0KGluZGV4KSB7XG4gICAgICByZXR1cm4gdGhpcy5lbGVtZW50c1tpbmRleF07XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQHBhcmFtZXRlciB7bnVtYmVyfSBpbmRleFxuICAgICAqIEBtZW1iZXJvZiBBcnJheVNsaWNlLnByb3RvdHlwZVxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdnZXRWYWx1ZScsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIGdldFZhbHVlKGluZGV4KSB7XG4gICAgICB2YXIgZWxlbWVudCA9IHRoaXMuZWxlbWVudHNbaW5kZXhdO1xuXG4gICAgICBpZiAoZWxlbWVudCkge1xuICAgICAgICByZXR1cm4gZWxlbWVudC50b1ZhbHVlKCk7XG4gICAgICB9XG5cbiAgICAgIHJldHVybiB1bmRlZmluZWQ7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogUmV0dXJucyB0aGUgbnVtYmVyIG9mIGVsZW1lbnRzIGluIHRoZSBzbGljZVxuICAgICAqIEB0eXBlIG51bWJlclxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdsZW5ndGgnLFxuICAgIGdldDogZnVuY3Rpb24gZ2V0KCkge1xuICAgICAgcmV0dXJuIHRoaXMuZWxlbWVudHMubGVuZ3RoO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIFJldHVybnMgd2hldGhlciB0aGUgc2xpY2UgaXMgZW1wdHlcbiAgICAgKiBAdHlwZSBib29sZWFuXG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ2lzRW1wdHknLFxuICAgIGdldDogZnVuY3Rpb24gZ2V0KCkge1xuICAgICAgcmV0dXJuIHRoaXMuZWxlbWVudHMubGVuZ3RoID09PSAwO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIFJldHVybnMgdGhlIGZpcnN0IGVsZW1lbnQgaW4gdGhlIHNsaWNlIG9yIHVuZGVmaW5lZCBpZiB0aGUgc2xpY2UgaXMgZW1wdHlcbiAgICAgKiBAdHlwZSBFbGVtZW50XG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ2ZpcnN0JyxcbiAgICBnZXQ6IGZ1bmN0aW9uIGdldCgpIHtcbiAgICAgIHJldHVybiB0aGlzLmVsZW1lbnRzWzBdO1xuICAgIH1cbiAgfV0pO1xuXG4gIHJldHVybiBBcnJheVNsaWNlO1xufSgpO1xuXG5pZiAodHlwZW9mIFN5bWJvbCAhPT0gJ3VuZGVmaW5lZCcpIHtcbiAgQXJyYXlTbGljZS5wcm90b3R5cGVbU3ltYm9sLml0ZXJhdG9yXSA9IGZ1bmN0aW9uIHN5bWJvbCgpIHtcbiAgICByZXR1cm4gdGhpcy5lbGVtZW50c1tTeW1ib2wuaXRlcmF0b3JdKCk7XG4gIH07XG59XG5cbm1vZHVsZS5leHBvcnRzID0gQXJyYXlTbGljZTsiLCJcInVzZSBzdHJpY3RcIjtcblxudmFyIF9jcmVhdGVDbGFzcyA9IGZ1bmN0aW9uICgpIHsgZnVuY3Rpb24gZGVmaW5lUHJvcGVydGllcyh0YXJnZXQsIHByb3BzKSB7IGZvciAodmFyIGkgPSAwOyBpIDwgcHJvcHMubGVuZ3RoOyBpKyspIHsgdmFyIGRlc2NyaXB0b3IgPSBwcm9wc1tpXTsgZGVzY3JpcHRvci5lbnVtZXJhYmxlID0gZGVzY3JpcHRvci5lbnVtZXJhYmxlIHx8IGZhbHNlOyBkZXNjcmlwdG9yLmNvbmZpZ3VyYWJsZSA9IHRydWU7IGlmIChcInZhbHVlXCIgaW4gZGVzY3JpcHRvcikgZGVzY3JpcHRvci53cml0YWJsZSA9IHRydWU7IE9iamVjdC5kZWZpbmVQcm9wZXJ0eSh0YXJnZXQsIGRlc2NyaXB0b3Iua2V5LCBkZXNjcmlwdG9yKTsgfSB9IHJldHVybiBmdW5jdGlvbiAoQ29uc3RydWN0b3IsIHByb3RvUHJvcHMsIHN0YXRpY1Byb3BzKSB7IGlmIChwcm90b1Byb3BzKSBkZWZpbmVQcm9wZXJ0aWVzKENvbnN0cnVjdG9yLnByb3RvdHlwZSwgcHJvdG9Qcm9wcyk7IGlmIChzdGF0aWNQcm9wcykgZGVmaW5lUHJvcGVydGllcyhDb25zdHJ1Y3Rvciwgc3RhdGljUHJvcHMpOyByZXR1cm4gQ29uc3RydWN0b3I7IH07IH0oKTtcblxuZnVuY3Rpb24gX2NsYXNzQ2FsbENoZWNrKGluc3RhbmNlLCBDb25zdHJ1Y3RvcikgeyBpZiAoIShpbnN0YW5jZSBpbnN0YW5jZW9mIENvbnN0cnVjdG9yKSkgeyB0aHJvdyBuZXcgVHlwZUVycm9yKFwiQ2Fubm90IGNhbGwgYSBjbGFzcyBhcyBhIGZ1bmN0aW9uXCIpOyB9IH1cblxuLyoqXG4gKiBAY2xhc3NcbiAqXG4gKiBAcHJvcGVydHkge0VsZW1lbnR9IGtleVxuICogQHByb3BlcnR5IHtFbGVtZW50fSB2YWx1ZVxuICovXG52YXIgS2V5VmFsdWVQYWlyID0gZnVuY3Rpb24gKCkge1xuICBmdW5jdGlvbiBLZXlWYWx1ZVBhaXIoa2V5LCB2YWx1ZSkge1xuICAgIF9jbGFzc0NhbGxDaGVjayh0aGlzLCBLZXlWYWx1ZVBhaXIpO1xuXG4gICAgdGhpcy5rZXkgPSBrZXk7XG4gICAgdGhpcy52YWx1ZSA9IHZhbHVlO1xuICB9XG5cbiAgLyoqXG4gICAqIEByZXR1cm5zIHtLZXlWYWx1ZVBhaXJ9XG4gICAqL1xuXG5cbiAgX2NyZWF0ZUNsYXNzKEtleVZhbHVlUGFpciwgW3tcbiAgICBrZXk6IFwiY2xvbmVcIixcbiAgICB2YWx1ZTogZnVuY3Rpb24gY2xvbmUoKSB7XG4gICAgICB2YXIgY2xvbmUgPSBuZXcgS2V5VmFsdWVQYWlyKCk7XG5cbiAgICAgIGlmICh0aGlzLmtleSkge1xuICAgICAgICBjbG9uZS5rZXkgPSB0aGlzLmtleS5jbG9uZSgpO1xuICAgICAgfVxuXG4gICAgICBpZiAodGhpcy52YWx1ZSkge1xuICAgICAgICBjbG9uZS52YWx1ZSA9IHRoaXMudmFsdWUuY2xvbmUoKTtcbiAgICAgIH1cblxuICAgICAgcmV0dXJuIGNsb25lO1xuICAgIH1cbiAgfV0pO1xuXG4gIHJldHVybiBLZXlWYWx1ZVBhaXI7XG59KCk7XG5cbm1vZHVsZS5leHBvcnRzID0gS2V5VmFsdWVQYWlyOyIsIid1c2Ugc3RyaWN0JztcblxudmFyIF9jcmVhdGVDbGFzcyA9IGZ1bmN0aW9uICgpIHsgZnVuY3Rpb24gZGVmaW5lUHJvcGVydGllcyh0YXJnZXQsIHByb3BzKSB7IGZvciAodmFyIGkgPSAwOyBpIDwgcHJvcHMubGVuZ3RoOyBpKyspIHsgdmFyIGRlc2NyaXB0b3IgPSBwcm9wc1tpXTsgZGVzY3JpcHRvci5lbnVtZXJhYmxlID0gZGVzY3JpcHRvci5lbnVtZXJhYmxlIHx8IGZhbHNlOyBkZXNjcmlwdG9yLmNvbmZpZ3VyYWJsZSA9IHRydWU7IGlmIChcInZhbHVlXCIgaW4gZGVzY3JpcHRvcikgZGVzY3JpcHRvci53cml0YWJsZSA9IHRydWU7IE9iamVjdC5kZWZpbmVQcm9wZXJ0eSh0YXJnZXQsIGRlc2NyaXB0b3Iua2V5LCBkZXNjcmlwdG9yKTsgfSB9IHJldHVybiBmdW5jdGlvbiAoQ29uc3RydWN0b3IsIHByb3RvUHJvcHMsIHN0YXRpY1Byb3BzKSB7IGlmIChwcm90b1Byb3BzKSBkZWZpbmVQcm9wZXJ0aWVzKENvbnN0cnVjdG9yLnByb3RvdHlwZSwgcHJvdG9Qcm9wcyk7IGlmIChzdGF0aWNQcm9wcykgZGVmaW5lUHJvcGVydGllcyhDb25zdHJ1Y3Rvciwgc3RhdGljUHJvcHMpOyByZXR1cm4gQ29uc3RydWN0b3I7IH07IH0oKTtcblxuZnVuY3Rpb24gX2NsYXNzQ2FsbENoZWNrKGluc3RhbmNlLCBDb25zdHJ1Y3RvcikgeyBpZiAoIShpbnN0YW5jZSBpbnN0YW5jZW9mIENvbnN0cnVjdG9yKSkgeyB0aHJvdyBuZXcgVHlwZUVycm9yKFwiQ2Fubm90IGNhbGwgYSBjbGFzcyBhcyBhIGZ1bmN0aW9uXCIpOyB9IH1cblxudmFyIGlzTnVsbCA9IHJlcXVpcmUoJ2xvZGFzaC9pc051bGwnKTtcbnZhciBpc1N0cmluZyA9IHJlcXVpcmUoJ2xvZGFzaC9pc1N0cmluZycpO1xudmFyIGlzTnVtYmVyID0gcmVxdWlyZSgnbG9kYXNoL2lzTnVtYmVyJyk7XG52YXIgaXNCb29sZWFuID0gcmVxdWlyZSgnbG9kYXNoL2lzQm9vbGVhbicpO1xudmFyIGlzT2JqZWN0ID0gcmVxdWlyZSgnbG9kYXNoL2lzT2JqZWN0Jyk7XG5cbnZhciBKU09OU2VyaWFsaXNlciA9IHJlcXVpcmUoJy4vc2VyaWFsaXNlcnMvSlNPTlNlcmlhbGlzZXInKTtcbnZhciBlbGVtZW50cyA9IHJlcXVpcmUoJy4vZWxlbWVudHMnKTtcblxuLyoqXG4gKiBAY2xhc3NcbiAqXG4gKiBBIHJlZnJhY3QgZWxlbWVudCBpbXBsZW1lbnRhdGlvbiB3aXRoIGFuIGV4dGVuc2libGUgbmFtZXNwYWNlLCBhYmxlIHRvXG4gKiBsb2FkIG90aGVyIG5hbWVzcGFjZXMgaW50byBpdC5cbiAqXG4gKiBUaGUgbmFtZXNwYWNlIGFsbG93cyB5b3UgdG8gcmVnaXN0ZXIgeW91ciBvd24gY2xhc3NlcyB0byBiZSBpbnN0YW50aWF0ZWRcbiAqIHdoZW4gYSBwYXJ0aWN1bGFyIHJlZnJhY3QgZWxlbWVudCBpcyBlbmNvdW50ZXJlZCwgYW5kIGFsbG93cyB5b3UgdG8gc3BlY2lmeVxuICogd2hpY2ggZWxlbWVudHMgZ2V0IGluc3RhbnRpYXRlZCBmb3IgZXhpc3RpbmcgSmF2YXNjcmlwdCBvYmplY3RzLlxuICovXG5cbnZhciBOYW1lc3BhY2UgPSBmdW5jdGlvbiAoKSB7XG4gIGZ1bmN0aW9uIE5hbWVzcGFjZShvcHRpb25zKSB7XG4gICAgX2NsYXNzQ2FsbENoZWNrKHRoaXMsIE5hbWVzcGFjZSk7XG5cbiAgICB0aGlzLmVsZW1lbnRNYXAgPSB7fTtcbiAgICB0aGlzLmVsZW1lbnREZXRlY3Rpb24gPSBbXTtcbiAgICB0aGlzLkVsZW1lbnQgPSBlbGVtZW50cy5FbGVtZW50O1xuICAgIHRoaXMuS2V5VmFsdWVQYWlyID0gZWxlbWVudHMuS2V5VmFsdWVQYWlyO1xuXG4gICAgaWYgKCFvcHRpb25zIHx8ICFvcHRpb25zLm5vRGVmYXVsdCkge1xuICAgICAgdGhpcy51c2VEZWZhdWx0KCk7XG4gICAgfVxuXG4gICAgLy8gVGhlc2UgcHJvdmlkZSB0aGUgZGVmYXVsdHMgZm9yIG5ldyBlbGVtZW50cy5cbiAgICB0aGlzLl9hdHRyaWJ1dGVFbGVtZW50S2V5cyA9IFtdO1xuICAgIHRoaXMuX2F0dHJpYnV0ZUVsZW1lbnRBcnJheUtleXMgPSBbXTtcbiAgfVxuXG4gIC8qKlxuICAgKiBVc2UgYSBuYW1lc3BhY2UgcGx1Z2luIG9yIGxvYWQgYSBnZW5lcmljIHBsdWdpbi5cbiAgICpcbiAgICogQHBhcmFtIHBsdWdpblxuICAgKi9cblxuXG4gIF9jcmVhdGVDbGFzcyhOYW1lc3BhY2UsIFt7XG4gICAga2V5OiAndXNlJyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gdXNlKHBsdWdpbikge1xuICAgICAgaWYgKHBsdWdpbi5uYW1lc3BhY2UpIHtcbiAgICAgICAgcGx1Z2luLm5hbWVzcGFjZSh7IGJhc2U6IHRoaXMgfSk7XG4gICAgICB9XG4gICAgICBpZiAocGx1Z2luLmxvYWQpIHtcbiAgICAgICAgcGx1Z2luLmxvYWQoeyBiYXNlOiB0aGlzIH0pO1xuICAgICAgfVxuICAgICAgcmV0dXJuIHRoaXM7XG4gICAgfVxuXG4gICAgLypcbiAgICAgKiBVc2UgdGhlIGRlZmF1bHQgbmFtZXNwYWNlLiBUaGlzIHByZWxvYWRzIGFsbCB0aGUgZGVmYXVsdCBlbGVtZW50c1xuICAgICAqIGludG8gdGhpcyByZWdpc3RyeSBpbnN0YW5jZS5cbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAndXNlRGVmYXVsdCcsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIHVzZURlZmF1bHQoKSB7XG4gICAgICAvLyBTZXQgdXAgY2xhc3NlcyBmb3IgZGVmYXVsdCBlbGVtZW50c1xuICAgICAgdGhpcy5yZWdpc3RlcignbnVsbCcsIGVsZW1lbnRzLk51bGxFbGVtZW50KS5yZWdpc3Rlcignc3RyaW5nJywgZWxlbWVudHMuU3RyaW5nRWxlbWVudCkucmVnaXN0ZXIoJ251bWJlcicsIGVsZW1lbnRzLk51bWJlckVsZW1lbnQpLnJlZ2lzdGVyKCdib29sZWFuJywgZWxlbWVudHMuQm9vbGVhbkVsZW1lbnQpLnJlZ2lzdGVyKCdhcnJheScsIGVsZW1lbnRzLkFycmF5RWxlbWVudCkucmVnaXN0ZXIoJ29iamVjdCcsIGVsZW1lbnRzLk9iamVjdEVsZW1lbnQpLnJlZ2lzdGVyKCdtZW1iZXInLCBlbGVtZW50cy5NZW1iZXJFbGVtZW50KS5yZWdpc3RlcigncmVmJywgZWxlbWVudHMuUmVmRWxlbWVudCkucmVnaXN0ZXIoJ2xpbmsnLCBlbGVtZW50cy5MaW5rRWxlbWVudCk7XG5cbiAgICAgIC8vIEFkZCBpbnN0YW5jZSBkZXRlY3Rpb24gZnVuY3Rpb25zIHRvIGNvbnZlcnQgZXhpc3Rpbmcgb2JqZWN0cyBpbnRvXG4gICAgICAvLyB0aGUgY29ycmVzcG9uZGluZyByZWZyYWN0IGVsZW1lbnRzLlxuICAgICAgdGhpcy5kZXRlY3QoaXNOdWxsLCBlbGVtZW50cy5OdWxsRWxlbWVudCwgZmFsc2UpLmRldGVjdChpc1N0cmluZywgZWxlbWVudHMuU3RyaW5nRWxlbWVudCwgZmFsc2UpLmRldGVjdChpc051bWJlciwgZWxlbWVudHMuTnVtYmVyRWxlbWVudCwgZmFsc2UpLmRldGVjdChpc0Jvb2xlYW4sIGVsZW1lbnRzLkJvb2xlYW5FbGVtZW50LCBmYWxzZSkuZGV0ZWN0KEFycmF5LmlzQXJyYXksIGVsZW1lbnRzLkFycmF5RWxlbWVudCwgZmFsc2UpLmRldGVjdChpc09iamVjdCwgZWxlbWVudHMuT2JqZWN0RWxlbWVudCwgZmFsc2UpO1xuXG4gICAgICByZXR1cm4gdGhpcztcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBSZWdpc3RlciBhIG5ldyBlbGVtZW50IGNsYXNzIGZvciBhbiBlbGVtZW50LlxuICAgICAqXG4gICAgICogQHBhcmFtIHtzdHJpbmd9IG5hbWVcbiAgICAgKiBAcGFyYW0gZWxlbWVudENsYXNzXG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ3JlZ2lzdGVyJyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gcmVnaXN0ZXIobmFtZSwgRWxlbWVudENsYXNzKSB7XG4gICAgICB0aGlzLl9lbGVtZW50cyA9IHVuZGVmaW5lZDtcbiAgICAgIHRoaXMuZWxlbWVudE1hcFtuYW1lXSA9IEVsZW1lbnRDbGFzcztcbiAgICAgIHJldHVybiB0aGlzO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIFVucmVnaXN0ZXIgYSBwcmV2aW91c2x5IHJlZ2lzdGVyZWQgY2xhc3MgZm9yIGFuIGVsZW1lbnQuXG4gICAgICpcbiAgICAgKiBAcGFyYW0ge3N0cmluZ30gbmFtZVxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICd1bnJlZ2lzdGVyJyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gdW5yZWdpc3RlcihuYW1lKSB7XG4gICAgICB0aGlzLl9lbGVtZW50cyA9IHVuZGVmaW5lZDtcbiAgICAgIGRlbGV0ZSB0aGlzLmVsZW1lbnRNYXBbbmFtZV07XG4gICAgICByZXR1cm4gdGhpcztcbiAgICB9XG5cbiAgICAvKlxuICAgICAqIEFkZCBhIG5ldyBkZXRlY3Rpb24gZnVuY3Rpb24gdG8gZGV0ZXJtaW5lIHdoaWNoIGVsZW1lbnRcbiAgICAgKiBjbGFzcyB0byB1c2Ugd2hlbiBjb252ZXJ0aW5nIGV4aXN0aW5nIGpzIGluc3RhbmNlcyBpbnRvXG4gICAgICogcmVmcmFjdCBlbGVtZW50LlxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdkZXRlY3QnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBkZXRlY3QodGVzdCwgRWxlbWVudENsYXNzLCBnaXZlblByZXBlbmQpIHtcbiAgICAgIHZhciBwcmVwZW5kID0gZ2l2ZW5QcmVwZW5kID09PSB1bmRlZmluZWQgPyB0cnVlIDogZ2l2ZW5QcmVwZW5kO1xuXG4gICAgICBpZiAocHJlcGVuZCkge1xuICAgICAgICB0aGlzLmVsZW1lbnREZXRlY3Rpb24udW5zaGlmdChbdGVzdCwgRWxlbWVudENsYXNzXSk7XG4gICAgICB9IGVsc2Uge1xuICAgICAgICB0aGlzLmVsZW1lbnREZXRlY3Rpb24ucHVzaChbdGVzdCwgRWxlbWVudENsYXNzXSk7XG4gICAgICB9XG5cbiAgICAgIHJldHVybiB0aGlzO1xuICAgIH1cblxuICAgIC8qXG4gICAgICogQ29udmVydCBhbiBleGlzdGluZyBKYXZhc2NyaXB0IG9iamVjdCBpbnRvIHJlZnJhY3QgZWxlbWVudCBpbnN0YW5jZXMsIHdoaWNoXG4gICAgICogY2FuIGJlIGZ1cnRoZXIgcHJvY2Vzc2VkIG9yIHNlcmlhbGl6ZWQgaW50byByZWZyYWN0LlxuICAgICAqIElmIHRoZSBpdGVtIHBhc3NlZCBpbiBpcyBhbHJlYWR5IHJlZnJhY3RlZCwgdGhlbiBpdCBpcyByZXR1cm5lZFxuICAgICAqIHVubW9kaWZpZWQuXG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ3RvRWxlbWVudCcsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIHRvRWxlbWVudCh2YWx1ZSkge1xuICAgICAgaWYgKHZhbHVlIGluc3RhbmNlb2YgdGhpcy5FbGVtZW50KSB7XG4gICAgICAgIHJldHVybiB2YWx1ZTtcbiAgICAgIH1cblxuICAgICAgdmFyIGVsZW1lbnQgPSB2b2lkIDA7XG5cbiAgICAgIGZvciAodmFyIGkgPSAwOyBpIDwgdGhpcy5lbGVtZW50RGV0ZWN0aW9uLmxlbmd0aDsgaSArPSAxKSB7XG4gICAgICAgIHZhciB0ZXN0ID0gdGhpcy5lbGVtZW50RGV0ZWN0aW9uW2ldWzBdO1xuICAgICAgICB2YXIgRWxlbWVudENsYXNzID0gdGhpcy5lbGVtZW50RGV0ZWN0aW9uW2ldWzFdO1xuXG4gICAgICAgIGlmICh0ZXN0KHZhbHVlKSkge1xuICAgICAgICAgIGVsZW1lbnQgPSBuZXcgRWxlbWVudENsYXNzKHZhbHVlKTtcbiAgICAgICAgICBicmVhaztcbiAgICAgICAgfVxuICAgICAgfVxuXG4gICAgICByZXR1cm4gZWxlbWVudDtcbiAgICB9XG5cbiAgICAvKlxuICAgICAqIEdldCBhbiBlbGVtZW50IGNsYXNzIGdpdmVuIGFuIGVsZW1lbnQgbmFtZS5cbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAnZ2V0RWxlbWVudENsYXNzJyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gZ2V0RWxlbWVudENsYXNzKGVsZW1lbnQpIHtcbiAgICAgIHZhciBFbGVtZW50Q2xhc3MgPSB0aGlzLmVsZW1lbnRNYXBbZWxlbWVudF07XG5cbiAgICAgIGlmIChFbGVtZW50Q2xhc3MgPT09IHVuZGVmaW5lZCkge1xuICAgICAgICAvLyBGYWxsIGJhY2sgdG8gdGhlIGJhc2UgZWxlbWVudC4gV2UgbWF5IG5vdCBrbm93IHdoYXRcbiAgICAgICAgLy8gdG8gZG8gd2l0aCB0aGUgYGNvbnRlbnRgLCBidXQgZG93bnN0cmVhbSBzb2Z0d2FyZVxuICAgICAgICAvLyBtYXkga25vdy5cbiAgICAgICAgcmV0dXJuIHRoaXMuRWxlbWVudDtcbiAgICAgIH1cblxuICAgICAgcmV0dXJuIEVsZW1lbnRDbGFzcztcbiAgICB9XG5cbiAgICAvKlxuICAgICAqIENvbnZlcnQgYSByZWZyYWN0IGRvY3VtZW50IGludG8gcmVmcmFjdCBlbGVtZW50IGluc3RhbmNlcy5cbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAnZnJvbVJlZnJhY3QnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBmcm9tUmVmcmFjdChkb2MpIHtcbiAgICAgIHJldHVybiB0aGlzLnNlcmlhbGlzZXIuZGVzZXJpYWxpc2UoZG9jKTtcbiAgICB9XG5cbiAgICAvKlxuICAgICAqIENvbnZlcnQgYW4gZWxlbWVudCB0byBhIFJlZnJhY3RlZCBKU09OIG9iamVjdC5cbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAndG9SZWZyYWN0JyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gdG9SZWZyYWN0KGVsZW1lbnQpIHtcbiAgICAgIHJldHVybiB0aGlzLnNlcmlhbGlzZXIuc2VyaWFsaXNlKGVsZW1lbnQpO1xuICAgIH1cblxuICAgIC8qXG4gICAgICogR2V0IGFuIG9iamVjdCB0aGF0IGNvbnRhaW5zIGFsbCByZWdpc3RlcmVkIGVsZW1lbnQgY2xhc3Nlcywgd2hlcmVcbiAgICAgKiB0aGUga2V5IGlzIHRoZSBQYXNjYWxDYXNlZCBlbGVtZW50IG5hbWUgYW5kIHRoZSB2YWx1ZSBpcyB0aGUgY2xhc3MuXG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ2VsZW1lbnRzJyxcbiAgICBnZXQ6IGZ1bmN0aW9uIGdldCgpIHtcbiAgICAgIHZhciBfdGhpcyA9IHRoaXM7XG5cbiAgICAgIGlmICh0aGlzLl9lbGVtZW50cyA9PT0gdW5kZWZpbmVkKSB7XG4gICAgICAgIHRoaXMuX2VsZW1lbnRzID0ge1xuICAgICAgICAgIEVsZW1lbnQ6IHRoaXMuRWxlbWVudFxuICAgICAgICB9O1xuXG4gICAgICAgIE9iamVjdC5rZXlzKHRoaXMuZWxlbWVudE1hcCkuZm9yRWFjaChmdW5jdGlvbiAobmFtZSkge1xuICAgICAgICAgIC8vIEN1cnJlbnRseSwgYWxsIHJlZ2lzdGVyZWQgZWxlbWVudCB0eXBlcyB1c2UgYSBjYW1lbENhc2VOYW1lLlxuICAgICAgICAgIC8vIENvbnZlcnRpbmcgdG8gUGFzY2FsQ2FzZSBpcyBhcyBzaW1wbGUgYXMgdXBwZXItY2FzaW5nIHRoZSBmaXJzdFxuICAgICAgICAgIC8vIGxldHRlci5cbiAgICAgICAgICB2YXIgcGFzY2FsID0gbmFtZVswXS50b1VwcGVyQ2FzZSgpICsgbmFtZS5zdWJzdHIoMSk7XG4gICAgICAgICAgX3RoaXMuX2VsZW1lbnRzW3Bhc2NhbF0gPSBfdGhpcy5lbGVtZW50TWFwW25hbWVdO1xuICAgICAgICB9KTtcbiAgICAgIH1cblxuICAgICAgcmV0dXJuIHRoaXMuX2VsZW1lbnRzO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIENvbnZpbmllbmNlIG1ldGhvZCBmb3IgZ2V0dGluZyBhIEpTT04gU2VyaWFsaXNlciBjb25maWd1cmVkIHdpdGggdGhlXG4gICAgICogY3VycmVudCBuYW1lc3BhY2VcbiAgICAgKlxuICAgICAqIEB0eXBlIEpTT05TZXJpYWxpc2VyXG4gICAgICogQHJlYWRvbmx5XG4gICAgICpcbiAgICAgKiBAbWVtYmVyb2YgTmFtZXNwYWNlLnByb3RvdHlwZVxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdzZXJpYWxpc2VyJyxcbiAgICBnZXQ6IGZ1bmN0aW9uIGdldCgpIHtcbiAgICAgIHJldHVybiBuZXcgSlNPTlNlcmlhbGlzZXIodGhpcyk7XG4gICAgfVxuICB9XSk7XG5cbiAgcmV0dXJuIE5hbWVzcGFjZTtcbn0oKTtcblxuSlNPTlNlcmlhbGlzZXIucHJvdG90eXBlLk5hbWVzcGFjZSA9IE5hbWVzcGFjZTtcblxubW9kdWxlLmV4cG9ydHMgPSBOYW1lc3BhY2U7IiwiJ3VzZSBzdHJpY3QnO1xuXG52YXIgX2NyZWF0ZUNsYXNzID0gZnVuY3Rpb24gKCkgeyBmdW5jdGlvbiBkZWZpbmVQcm9wZXJ0aWVzKHRhcmdldCwgcHJvcHMpIHsgZm9yICh2YXIgaSA9IDA7IGkgPCBwcm9wcy5sZW5ndGg7IGkrKykgeyB2YXIgZGVzY3JpcHRvciA9IHByb3BzW2ldOyBkZXNjcmlwdG9yLmVudW1lcmFibGUgPSBkZXNjcmlwdG9yLmVudW1lcmFibGUgfHwgZmFsc2U7IGRlc2NyaXB0b3IuY29uZmlndXJhYmxlID0gdHJ1ZTsgaWYgKFwidmFsdWVcIiBpbiBkZXNjcmlwdG9yKSBkZXNjcmlwdG9yLndyaXRhYmxlID0gdHJ1ZTsgT2JqZWN0LmRlZmluZVByb3BlcnR5KHRhcmdldCwgZGVzY3JpcHRvci5rZXksIGRlc2NyaXB0b3IpOyB9IH0gcmV0dXJuIGZ1bmN0aW9uIChDb25zdHJ1Y3RvciwgcHJvdG9Qcm9wcywgc3RhdGljUHJvcHMpIHsgaWYgKHByb3RvUHJvcHMpIGRlZmluZVByb3BlcnRpZXMoQ29uc3RydWN0b3IucHJvdG90eXBlLCBwcm90b1Byb3BzKTsgaWYgKHN0YXRpY1Byb3BzKSBkZWZpbmVQcm9wZXJ0aWVzKENvbnN0cnVjdG9yLCBzdGF0aWNQcm9wcyk7IHJldHVybiBDb25zdHJ1Y3RvcjsgfTsgfSgpO1xuXG5mdW5jdGlvbiBfY2xhc3NDYWxsQ2hlY2soaW5zdGFuY2UsIENvbnN0cnVjdG9yKSB7IGlmICghKGluc3RhbmNlIGluc3RhbmNlb2YgQ29uc3RydWN0b3IpKSB7IHRocm93IG5ldyBUeXBlRXJyb3IoXCJDYW5ub3QgY2FsbCBhIGNsYXNzIGFzIGEgZnVuY3Rpb25cIik7IH0gfVxuXG5mdW5jdGlvbiBfcG9zc2libGVDb25zdHJ1Y3RvclJldHVybihzZWxmLCBjYWxsKSB7IGlmICghc2VsZikgeyB0aHJvdyBuZXcgUmVmZXJlbmNlRXJyb3IoXCJ0aGlzIGhhc24ndCBiZWVuIGluaXRpYWxpc2VkIC0gc3VwZXIoKSBoYXNuJ3QgYmVlbiBjYWxsZWRcIik7IH0gcmV0dXJuIGNhbGwgJiYgKHR5cGVvZiBjYWxsID09PSBcIm9iamVjdFwiIHx8IHR5cGVvZiBjYWxsID09PSBcImZ1bmN0aW9uXCIpID8gY2FsbCA6IHNlbGY7IH1cblxuZnVuY3Rpb24gX2luaGVyaXRzKHN1YkNsYXNzLCBzdXBlckNsYXNzKSB7IGlmICh0eXBlb2Ygc3VwZXJDbGFzcyAhPT0gXCJmdW5jdGlvblwiICYmIHN1cGVyQ2xhc3MgIT09IG51bGwpIHsgdGhyb3cgbmV3IFR5cGVFcnJvcihcIlN1cGVyIGV4cHJlc3Npb24gbXVzdCBlaXRoZXIgYmUgbnVsbCBvciBhIGZ1bmN0aW9uLCBub3QgXCIgKyB0eXBlb2Ygc3VwZXJDbGFzcyk7IH0gc3ViQ2xhc3MucHJvdG90eXBlID0gT2JqZWN0LmNyZWF0ZShzdXBlckNsYXNzICYmIHN1cGVyQ2xhc3MucHJvdG90eXBlLCB7IGNvbnN0cnVjdG9yOiB7IHZhbHVlOiBzdWJDbGFzcywgZW51bWVyYWJsZTogZmFsc2UsIHdyaXRhYmxlOiB0cnVlLCBjb25maWd1cmFibGU6IHRydWUgfSB9KTsgaWYgKHN1cGVyQ2xhc3MpIE9iamVjdC5zZXRQcm90b3R5cGVPZiA/IE9iamVjdC5zZXRQcm90b3R5cGVPZihzdWJDbGFzcywgc3VwZXJDbGFzcykgOiBzdWJDbGFzcy5fX3Byb3RvX18gPSBzdXBlckNsYXNzOyB9XG5cbnZhciBuZWdhdGUgPSByZXF1aXJlKCdsb2Rhc2gvbmVnYXRlJyk7XG52YXIgQXJyYXlTbGljZSA9IHJlcXVpcmUoJy4vQXJyYXlTbGljZScpO1xuXG4vKipcbiAqL1xuXG52YXIgT2JqZWN0U2xpY2UgPSBmdW5jdGlvbiAoX0FycmF5U2xpY2UpIHtcbiAgX2luaGVyaXRzKE9iamVjdFNsaWNlLCBfQXJyYXlTbGljZSk7XG5cbiAgZnVuY3Rpb24gT2JqZWN0U2xpY2UoKSB7XG4gICAgX2NsYXNzQ2FsbENoZWNrKHRoaXMsIE9iamVjdFNsaWNlKTtcblxuICAgIHJldHVybiBfcG9zc2libGVDb25zdHJ1Y3RvclJldHVybih0aGlzLCAoT2JqZWN0U2xpY2UuX19wcm90b19fIHx8IE9iamVjdC5nZXRQcm90b3R5cGVPZihPYmplY3RTbGljZSkpLmFwcGx5KHRoaXMsIGFyZ3VtZW50cykpO1xuICB9XG5cbiAgX2NyZWF0ZUNsYXNzKE9iamVjdFNsaWNlLCBbe1xuICAgIGtleTogJ21hcCcsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIG1hcChjYWxsYmFjaywgdGhpc0FyZykge1xuICAgICAgcmV0dXJuIHRoaXMuZWxlbWVudHMubWFwKGZ1bmN0aW9uIChtZW1iZXIpIHtcbiAgICAgICAgcmV0dXJuIGNhbGxiYWNrLmJpbmQodGhpc0FyZykobWVtYmVyLnZhbHVlLCBtZW1iZXIua2V5LCBtZW1iZXIpO1xuICAgICAgfSk7XG4gICAgfVxuICB9LCB7XG4gICAga2V5OiAnZmlsdGVyJyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gZmlsdGVyKGNhbGxiYWNrLCB0aGlzQXJnKSB7XG4gICAgICByZXR1cm4gbmV3IE9iamVjdFNsaWNlKHRoaXMuZWxlbWVudHMuZmlsdGVyKGZ1bmN0aW9uIChtZW1iZXIpIHtcbiAgICAgICAgcmV0dXJuIGNhbGxiYWNrLmJpbmQodGhpc0FyZykobWVtYmVyLnZhbHVlLCBtZW1iZXIua2V5LCBtZW1iZXIpO1xuICAgICAgfSkpO1xuICAgIH1cbiAgfSwge1xuICAgIGtleTogJ3JlamVjdCcsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIHJlamVjdChjYWxsYmFjaywgdGhpc0FyZykge1xuICAgICAgcmV0dXJuIHRoaXMuZmlsdGVyKG5lZ2F0ZShjYWxsYmFjay5iaW5kKHRoaXNBcmcpKSk7XG4gICAgfVxuICB9LCB7XG4gICAga2V5OiAnZm9yRWFjaCcsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIGZvckVhY2goY2FsbGJhY2ssIHRoaXNBcmcpIHtcbiAgICAgIHJldHVybiB0aGlzLmVsZW1lbnRzLmZvckVhY2goZnVuY3Rpb24gKG1lbWJlciwgaW5kZXgpIHtcbiAgICAgICAgY2FsbGJhY2suYmluZCh0aGlzQXJnKShtZW1iZXIudmFsdWUsIG1lbWJlci5rZXksIG1lbWJlciwgaW5kZXgpO1xuICAgICAgfSk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQHJldHVybnMge2FycmF5fVxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdrZXlzJyxcbiAgICB2YWx1ZTogZnVuY3Rpb24ga2V5cygpIHtcbiAgICAgIHJldHVybiB0aGlzLm1hcChmdW5jdGlvbiAodmFsdWUsIGtleSkge1xuICAgICAgICByZXR1cm4ga2V5LnRvVmFsdWUoKTtcbiAgICAgIH0pO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEByZXR1cm5zIHthcnJheX1cbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAndmFsdWVzJyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gdmFsdWVzKCkge1xuICAgICAgcmV0dXJuIHRoaXMubWFwKGZ1bmN0aW9uICh2YWx1ZSkge1xuICAgICAgICByZXR1cm4gdmFsdWUudG9WYWx1ZSgpO1xuICAgICAgfSk7XG4gICAgfVxuICB9XSk7XG5cbiAgcmV0dXJuIE9iamVjdFNsaWNlO1xufShBcnJheVNsaWNlKTtcblxubW9kdWxlLmV4cG9ydHMgPSBPYmplY3RTbGljZTsiLCIndXNlIHN0cmljdCc7XG5cbnZhciBfdHlwZW9mID0gdHlwZW9mIFN5bWJvbCA9PT0gXCJmdW5jdGlvblwiICYmIHR5cGVvZiBTeW1ib2wuaXRlcmF0b3IgPT09IFwic3ltYm9sXCIgPyBmdW5jdGlvbiAob2JqKSB7IHJldHVybiB0eXBlb2Ygb2JqOyB9IDogZnVuY3Rpb24gKG9iaikgeyByZXR1cm4gb2JqICYmIHR5cGVvZiBTeW1ib2wgPT09IFwiZnVuY3Rpb25cIiAmJiBvYmouY29uc3RydWN0b3IgPT09IFN5bWJvbCAmJiBvYmogIT09IFN5bWJvbC5wcm90b3R5cGUgPyBcInN5bWJvbFwiIDogdHlwZW9mIG9iajsgfTtcblxudmFyIEVsZW1lbnQgPSByZXF1aXJlKCcuL3ByaW1pdGl2ZXMvRWxlbWVudCcpO1xudmFyIE51bGxFbGVtZW50ID0gcmVxdWlyZSgnLi9wcmltaXRpdmVzL051bGxFbGVtZW50Jyk7XG52YXIgU3RyaW5nRWxlbWVudCA9IHJlcXVpcmUoJy4vcHJpbWl0aXZlcy9TdHJpbmdFbGVtZW50Jyk7XG52YXIgTnVtYmVyRWxlbWVudCA9IHJlcXVpcmUoJy4vcHJpbWl0aXZlcy9OdW1iZXJFbGVtZW50Jyk7XG52YXIgQm9vbGVhbkVsZW1lbnQgPSByZXF1aXJlKCcuL3ByaW1pdGl2ZXMvQm9vbGVhbkVsZW1lbnQnKTtcbnZhciBBcnJheUVsZW1lbnQgPSByZXF1aXJlKCcuL3ByaW1pdGl2ZXMvQXJyYXlFbGVtZW50Jyk7XG52YXIgTWVtYmVyRWxlbWVudCA9IHJlcXVpcmUoJy4vcHJpbWl0aXZlcy9NZW1iZXJFbGVtZW50Jyk7XG52YXIgT2JqZWN0RWxlbWVudCA9IHJlcXVpcmUoJy4vcHJpbWl0aXZlcy9PYmplY3RFbGVtZW50Jyk7XG52YXIgTGlua0VsZW1lbnQgPSByZXF1aXJlKCcuL2VsZW1lbnRzL0xpbmtFbGVtZW50Jyk7XG52YXIgUmVmRWxlbWVudCA9IHJlcXVpcmUoJy4vZWxlbWVudHMvUmVmRWxlbWVudCcpO1xuXG52YXIgQXJyYXlTbGljZSA9IHJlcXVpcmUoJy4vQXJyYXlTbGljZScpO1xudmFyIE9iamVjdFNsaWNlID0gcmVxdWlyZSgnLi9PYmplY3RTbGljZScpO1xuXG52YXIgS2V5VmFsdWVQYWlyID0gcmVxdWlyZSgnLi9LZXlWYWx1ZVBhaXInKTtcblxuLyoqXG4gKiBSZWZyYWN0cyBhIEpTT04gdHlwZSB0byBtaW5pbSBlbGVtZW50c1xuICogQHBhcmFtIHZhbHVlXG4gKiBAcmV0dXJucyB7RWxlbWVudH1cbiAqL1xuZnVuY3Rpb24gcmVmcmFjdCh2YWx1ZSkge1xuICBpZiAodmFsdWUgaW5zdGFuY2VvZiBFbGVtZW50KSB7XG4gICAgcmV0dXJuIHZhbHVlO1xuICB9XG5cbiAgaWYgKHR5cGVvZiB2YWx1ZSA9PT0gJ3N0cmluZycpIHtcbiAgICByZXR1cm4gbmV3IFN0cmluZ0VsZW1lbnQodmFsdWUpO1xuICB9XG5cbiAgaWYgKHR5cGVvZiB2YWx1ZSA9PT0gJ251bWJlcicpIHtcbiAgICByZXR1cm4gbmV3IE51bWJlckVsZW1lbnQodmFsdWUpO1xuICB9XG5cbiAgaWYgKHR5cGVvZiB2YWx1ZSA9PT0gJ2Jvb2xlYW4nKSB7XG4gICAgcmV0dXJuIG5ldyBCb29sZWFuRWxlbWVudCh2YWx1ZSk7XG4gIH1cblxuICBpZiAodmFsdWUgPT09IG51bGwpIHtcbiAgICByZXR1cm4gbmV3IE51bGxFbGVtZW50KCk7XG4gIH1cblxuICBpZiAoQXJyYXkuaXNBcnJheSh2YWx1ZSkpIHtcbiAgICByZXR1cm4gbmV3IEFycmF5RWxlbWVudCh2YWx1ZS5tYXAocmVmcmFjdCkpO1xuICB9XG5cbiAgaWYgKCh0eXBlb2YgdmFsdWUgPT09ICd1bmRlZmluZWQnID8gJ3VuZGVmaW5lZCcgOiBfdHlwZW9mKHZhbHVlKSkgPT09ICdvYmplY3QnKSB7XG4gICAgdmFyIGVsZW1lbnQgPSBuZXcgT2JqZWN0RWxlbWVudCh2YWx1ZSk7XG4gICAgcmV0dXJuIGVsZW1lbnQ7XG4gIH1cblxuICByZXR1cm4gdmFsdWU7XG59XG5cbkVsZW1lbnQucHJvdG90eXBlLk9iamVjdEVsZW1lbnQgPSBPYmplY3RFbGVtZW50O1xuRWxlbWVudC5wcm90b3R5cGUuUmVmRWxlbWVudCA9IFJlZkVsZW1lbnQ7XG5FbGVtZW50LnByb3RvdHlwZS5NZW1iZXJFbGVtZW50ID0gTWVtYmVyRWxlbWVudDtcblxuRWxlbWVudC5wcm90b3R5cGUucmVmcmFjdCA9IHJlZnJhY3Q7XG5BcnJheVNsaWNlLnByb3RvdHlwZS5yZWZyYWN0ID0gcmVmcmFjdDtcblxuLyoqXG4gKiBDb250YWlucyBhbGwgb2YgdGhlIGVsZW1lbnQgY2xhc3NlcywgYW5kIHJlbGF0ZWQgc3RydWN0dXJlcyBhbmQgbWV0aG9kc1xuICogZm9yIGhhbmRsaW5nIHdpdGggZWxlbWVudCBpbnN0YW5jZXMuXG4gKi9cbm1vZHVsZS5leHBvcnRzID0ge1xuICBFbGVtZW50OiBFbGVtZW50LFxuICBOdWxsRWxlbWVudDogTnVsbEVsZW1lbnQsXG4gIFN0cmluZ0VsZW1lbnQ6IFN0cmluZ0VsZW1lbnQsXG4gIE51bWJlckVsZW1lbnQ6IE51bWJlckVsZW1lbnQsXG4gIEJvb2xlYW5FbGVtZW50OiBCb29sZWFuRWxlbWVudCxcbiAgQXJyYXlFbGVtZW50OiBBcnJheUVsZW1lbnQsXG4gIE1lbWJlckVsZW1lbnQ6IE1lbWJlckVsZW1lbnQsXG4gIE9iamVjdEVsZW1lbnQ6IE9iamVjdEVsZW1lbnQsXG4gIExpbmtFbGVtZW50OiBMaW5rRWxlbWVudCxcbiAgUmVmRWxlbWVudDogUmVmRWxlbWVudCxcblxuICByZWZyYWN0OiByZWZyYWN0LFxuXG4gIEFycmF5U2xpY2U6IEFycmF5U2xpY2UsXG4gIE9iamVjdFNsaWNlOiBPYmplY3RTbGljZSxcbiAgS2V5VmFsdWVQYWlyOiBLZXlWYWx1ZVBhaXJcbn07IiwiJ3VzZSBzdHJpY3QnO1xuXG52YXIgX2NyZWF0ZUNsYXNzID0gZnVuY3Rpb24gKCkgeyBmdW5jdGlvbiBkZWZpbmVQcm9wZXJ0aWVzKHRhcmdldCwgcHJvcHMpIHsgZm9yICh2YXIgaSA9IDA7IGkgPCBwcm9wcy5sZW5ndGg7IGkrKykgeyB2YXIgZGVzY3JpcHRvciA9IHByb3BzW2ldOyBkZXNjcmlwdG9yLmVudW1lcmFibGUgPSBkZXNjcmlwdG9yLmVudW1lcmFibGUgfHwgZmFsc2U7IGRlc2NyaXB0b3IuY29uZmlndXJhYmxlID0gdHJ1ZTsgaWYgKFwidmFsdWVcIiBpbiBkZXNjcmlwdG9yKSBkZXNjcmlwdG9yLndyaXRhYmxlID0gdHJ1ZTsgT2JqZWN0LmRlZmluZVByb3BlcnR5KHRhcmdldCwgZGVzY3JpcHRvci5rZXksIGRlc2NyaXB0b3IpOyB9IH0gcmV0dXJuIGZ1bmN0aW9uIChDb25zdHJ1Y3RvciwgcHJvdG9Qcm9wcywgc3RhdGljUHJvcHMpIHsgaWYgKHByb3RvUHJvcHMpIGRlZmluZVByb3BlcnRpZXMoQ29uc3RydWN0b3IucHJvdG90eXBlLCBwcm90b1Byb3BzKTsgaWYgKHN0YXRpY1Byb3BzKSBkZWZpbmVQcm9wZXJ0aWVzKENvbnN0cnVjdG9yLCBzdGF0aWNQcm9wcyk7IHJldHVybiBDb25zdHJ1Y3RvcjsgfTsgfSgpO1xuXG5mdW5jdGlvbiBfY2xhc3NDYWxsQ2hlY2soaW5zdGFuY2UsIENvbnN0cnVjdG9yKSB7IGlmICghKGluc3RhbmNlIGluc3RhbmNlb2YgQ29uc3RydWN0b3IpKSB7IHRocm93IG5ldyBUeXBlRXJyb3IoXCJDYW5ub3QgY2FsbCBhIGNsYXNzIGFzIGEgZnVuY3Rpb25cIik7IH0gfVxuXG5mdW5jdGlvbiBfcG9zc2libGVDb25zdHJ1Y3RvclJldHVybihzZWxmLCBjYWxsKSB7IGlmICghc2VsZikgeyB0aHJvdyBuZXcgUmVmZXJlbmNlRXJyb3IoXCJ0aGlzIGhhc24ndCBiZWVuIGluaXRpYWxpc2VkIC0gc3VwZXIoKSBoYXNuJ3QgYmVlbiBjYWxsZWRcIik7IH0gcmV0dXJuIGNhbGwgJiYgKHR5cGVvZiBjYWxsID09PSBcIm9iamVjdFwiIHx8IHR5cGVvZiBjYWxsID09PSBcImZ1bmN0aW9uXCIpID8gY2FsbCA6IHNlbGY7IH1cblxuZnVuY3Rpb24gX2luaGVyaXRzKHN1YkNsYXNzLCBzdXBlckNsYXNzKSB7IGlmICh0eXBlb2Ygc3VwZXJDbGFzcyAhPT0gXCJmdW5jdGlvblwiICYmIHN1cGVyQ2xhc3MgIT09IG51bGwpIHsgdGhyb3cgbmV3IFR5cGVFcnJvcihcIlN1cGVyIGV4cHJlc3Npb24gbXVzdCBlaXRoZXIgYmUgbnVsbCBvciBhIGZ1bmN0aW9uLCBub3QgXCIgKyB0eXBlb2Ygc3VwZXJDbGFzcyk7IH0gc3ViQ2xhc3MucHJvdG90eXBlID0gT2JqZWN0LmNyZWF0ZShzdXBlckNsYXNzICYmIHN1cGVyQ2xhc3MucHJvdG90eXBlLCB7IGNvbnN0cnVjdG9yOiB7IHZhbHVlOiBzdWJDbGFzcywgZW51bWVyYWJsZTogZmFsc2UsIHdyaXRhYmxlOiB0cnVlLCBjb25maWd1cmFibGU6IHRydWUgfSB9KTsgaWYgKHN1cGVyQ2xhc3MpIE9iamVjdC5zZXRQcm90b3R5cGVPZiA/IE9iamVjdC5zZXRQcm90b3R5cGVPZihzdWJDbGFzcywgc3VwZXJDbGFzcykgOiBzdWJDbGFzcy5fX3Byb3RvX18gPSBzdXBlckNsYXNzOyB9XG5cbnZhciBFbGVtZW50ID0gcmVxdWlyZSgnLi4vcHJpbWl0aXZlcy9FbGVtZW50Jyk7XG5cbi8qKiBIeXBlcmxpbmtpbmcgTUFZIGJlIHVzZWQgdG8gbGluayB0byBvdGhlciByZXNvdXJjZXMsIHByb3ZpZGUgbGlua3MgdG9cbiAqIGluc3RydWN0aW9ucyBvbiBob3cgdG8gcHJvY2VzcyBhIGdpdmVuIGVsZW1lbnQgKGJ5IHdheSBvZiBhIHByb2ZpbGUgb3JcbiAqIG90aGVyIG1lYW5zKSwgYW5kIG1heSBiZSB1c2VkIHRvIHByb3ZpZGUgbWV0YSBkYXRhIGFib3V0IHRoZSBlbGVtZW50IGluXG4gKiB3aGljaCBpdCdzIGZvdW5kLiBUaGUgbWVhbmluZyBhbmQgcHVycG9zZSBvZiB0aGUgaHlwZXJsaW5rIGlzIGRlZmluZWQgYnlcbiAqIHRoZSBsaW5rIHJlbGF0aW9uIGFjY29yZGluZyB0byBSRkMgNTk4OC5cbiAqXG4gKiBAY2xhc3MgTGlua0VsZW1lbnRcbiAqXG4gKiBAcGFyYW0gY29udGVudFxuICogQHBhcmFtIG1ldGFcbiAqIEBwYXJhbSBhdHRyaWJ1dGVzXG4gKi9cbm1vZHVsZS5leHBvcnRzID0gZnVuY3Rpb24gKF9FbGVtZW50KSB7XG4gIF9pbmhlcml0cyhMaW5rRWxlbWVudCwgX0VsZW1lbnQpO1xuXG4gIGZ1bmN0aW9uIExpbmtFbGVtZW50KGNvbnRlbnQsIG1ldGEsIGF0dHJpYnV0ZXMpIHtcbiAgICBfY2xhc3NDYWxsQ2hlY2sodGhpcywgTGlua0VsZW1lbnQpO1xuXG4gICAgdmFyIF90aGlzID0gX3Bvc3NpYmxlQ29uc3RydWN0b3JSZXR1cm4odGhpcywgKExpbmtFbGVtZW50Ll9fcHJvdG9fXyB8fCBPYmplY3QuZ2V0UHJvdG90eXBlT2YoTGlua0VsZW1lbnQpKS5jYWxsKHRoaXMsIGNvbnRlbnQgfHwgW10sIG1ldGEsIGF0dHJpYnV0ZXMpKTtcblxuICAgIF90aGlzLmVsZW1lbnQgPSAnbGluayc7XG4gICAgcmV0dXJuIF90aGlzO1xuICB9XG5cbiAgLyoqXG4gICAqIFRoZSByZWxhdGlvbiBpZGVudGlmaWVyIGZvciB0aGUgbGluaywgYXMgZGVmaW5lZCBpbiBSRkMgNTk4OC5cbiAgICogQHR5cGUgU3RyaW5nRWxlbWVudFxuICAgKi9cblxuXG4gIF9jcmVhdGVDbGFzcyhMaW5rRWxlbWVudCwgW3tcbiAgICBrZXk6ICdyZWxhdGlvbicsXG4gICAgZ2V0OiBmdW5jdGlvbiBnZXQoKSB7XG4gICAgICByZXR1cm4gdGhpcy5hdHRyaWJ1dGVzLmdldCgncmVsYXRpb24nKTtcbiAgICB9LFxuICAgIHNldDogZnVuY3Rpb24gc2V0KHJlbGF0aW9uKSB7XG4gICAgICB0aGlzLmF0dHJpYnV0ZXMuc2V0KCdyZWxhdGlvbicsIHJlbGF0aW9uKTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBUaGUgVVJJIGZvciB0aGUgZ2l2ZW4gbGluay5cbiAgICAgKiBAdHlwZSBTdHJpbmdFbGVtZW50XG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ2hyZWYnLFxuICAgIGdldDogZnVuY3Rpb24gZ2V0KCkge1xuICAgICAgcmV0dXJuIHRoaXMuYXR0cmlidXRlcy5nZXQoJ2hyZWYnKTtcbiAgICB9LFxuICAgIHNldDogZnVuY3Rpb24gc2V0KGhyZWYpIHtcbiAgICAgIHRoaXMuYXR0cmlidXRlcy5zZXQoJ2hyZWYnLCBocmVmKTtcbiAgICB9XG4gIH1dKTtcblxuICByZXR1cm4gTGlua0VsZW1lbnQ7XG59KEVsZW1lbnQpOyIsIid1c2Ugc3RyaWN0JztcblxudmFyIF9jcmVhdGVDbGFzcyA9IGZ1bmN0aW9uICgpIHsgZnVuY3Rpb24gZGVmaW5lUHJvcGVydGllcyh0YXJnZXQsIHByb3BzKSB7IGZvciAodmFyIGkgPSAwOyBpIDwgcHJvcHMubGVuZ3RoOyBpKyspIHsgdmFyIGRlc2NyaXB0b3IgPSBwcm9wc1tpXTsgZGVzY3JpcHRvci5lbnVtZXJhYmxlID0gZGVzY3JpcHRvci5lbnVtZXJhYmxlIHx8IGZhbHNlOyBkZXNjcmlwdG9yLmNvbmZpZ3VyYWJsZSA9IHRydWU7IGlmIChcInZhbHVlXCIgaW4gZGVzY3JpcHRvcikgZGVzY3JpcHRvci53cml0YWJsZSA9IHRydWU7IE9iamVjdC5kZWZpbmVQcm9wZXJ0eSh0YXJnZXQsIGRlc2NyaXB0b3Iua2V5LCBkZXNjcmlwdG9yKTsgfSB9IHJldHVybiBmdW5jdGlvbiAoQ29uc3RydWN0b3IsIHByb3RvUHJvcHMsIHN0YXRpY1Byb3BzKSB7IGlmIChwcm90b1Byb3BzKSBkZWZpbmVQcm9wZXJ0aWVzKENvbnN0cnVjdG9yLnByb3RvdHlwZSwgcHJvdG9Qcm9wcyk7IGlmIChzdGF0aWNQcm9wcykgZGVmaW5lUHJvcGVydGllcyhDb25zdHJ1Y3Rvciwgc3RhdGljUHJvcHMpOyByZXR1cm4gQ29uc3RydWN0b3I7IH07IH0oKTtcblxuZnVuY3Rpb24gX2NsYXNzQ2FsbENoZWNrKGluc3RhbmNlLCBDb25zdHJ1Y3RvcikgeyBpZiAoIShpbnN0YW5jZSBpbnN0YW5jZW9mIENvbnN0cnVjdG9yKSkgeyB0aHJvdyBuZXcgVHlwZUVycm9yKFwiQ2Fubm90IGNhbGwgYSBjbGFzcyBhcyBhIGZ1bmN0aW9uXCIpOyB9IH1cblxuZnVuY3Rpb24gX3Bvc3NpYmxlQ29uc3RydWN0b3JSZXR1cm4oc2VsZiwgY2FsbCkgeyBpZiAoIXNlbGYpIHsgdGhyb3cgbmV3IFJlZmVyZW5jZUVycm9yKFwidGhpcyBoYXNuJ3QgYmVlbiBpbml0aWFsaXNlZCAtIHN1cGVyKCkgaGFzbid0IGJlZW4gY2FsbGVkXCIpOyB9IHJldHVybiBjYWxsICYmICh0eXBlb2YgY2FsbCA9PT0gXCJvYmplY3RcIiB8fCB0eXBlb2YgY2FsbCA9PT0gXCJmdW5jdGlvblwiKSA/IGNhbGwgOiBzZWxmOyB9XG5cbmZ1bmN0aW9uIF9pbmhlcml0cyhzdWJDbGFzcywgc3VwZXJDbGFzcykgeyBpZiAodHlwZW9mIHN1cGVyQ2xhc3MgIT09IFwiZnVuY3Rpb25cIiAmJiBzdXBlckNsYXNzICE9PSBudWxsKSB7IHRocm93IG5ldyBUeXBlRXJyb3IoXCJTdXBlciBleHByZXNzaW9uIG11c3QgZWl0aGVyIGJlIG51bGwgb3IgYSBmdW5jdGlvbiwgbm90IFwiICsgdHlwZW9mIHN1cGVyQ2xhc3MpOyB9IHN1YkNsYXNzLnByb3RvdHlwZSA9IE9iamVjdC5jcmVhdGUoc3VwZXJDbGFzcyAmJiBzdXBlckNsYXNzLnByb3RvdHlwZSwgeyBjb25zdHJ1Y3RvcjogeyB2YWx1ZTogc3ViQ2xhc3MsIGVudW1lcmFibGU6IGZhbHNlLCB3cml0YWJsZTogdHJ1ZSwgY29uZmlndXJhYmxlOiB0cnVlIH0gfSk7IGlmIChzdXBlckNsYXNzKSBPYmplY3Quc2V0UHJvdG90eXBlT2YgPyBPYmplY3Quc2V0UHJvdG90eXBlT2Yoc3ViQ2xhc3MsIHN1cGVyQ2xhc3MpIDogc3ViQ2xhc3MuX19wcm90b19fID0gc3VwZXJDbGFzczsgfVxuXG52YXIgRWxlbWVudCA9IHJlcXVpcmUoJy4uL3ByaW1pdGl2ZXMvRWxlbWVudCcpO1xuXG4vKipcbiAqIEBjbGFzcyBSZWZFbGVtZW50XG4gKlxuICogQHBhcmFtIGNvbnRlbnRcbiAqIEBwYXJhbSBtZXRhXG4gKiBAcGFyYW0gYXR0cmlidXRlc1xuICpcbiAqIEBleHRlbmRzIEVsZW1lbnRcbiAqL1xubW9kdWxlLmV4cG9ydHMgPSBmdW5jdGlvbiAoX0VsZW1lbnQpIHtcbiAgX2luaGVyaXRzKFJlZkVsZW1lbnQsIF9FbGVtZW50KTtcblxuICBmdW5jdGlvbiBSZWZFbGVtZW50KGNvbnRlbnQsIG1ldGEsIGF0dHJpYnV0ZXMpIHtcbiAgICBfY2xhc3NDYWxsQ2hlY2sodGhpcywgUmVmRWxlbWVudCk7XG5cbiAgICB2YXIgX3RoaXMgPSBfcG9zc2libGVDb25zdHJ1Y3RvclJldHVybih0aGlzLCAoUmVmRWxlbWVudC5fX3Byb3RvX18gfHwgT2JqZWN0LmdldFByb3RvdHlwZU9mKFJlZkVsZW1lbnQpKS5jYWxsKHRoaXMsIGNvbnRlbnQgfHwgW10sIG1ldGEsIGF0dHJpYnV0ZXMpKTtcblxuICAgIF90aGlzLmVsZW1lbnQgPSAncmVmJztcblxuICAgIGlmICghX3RoaXMucGF0aCkge1xuICAgICAgX3RoaXMucGF0aCA9ICdlbGVtZW50JztcbiAgICB9XG4gICAgcmV0dXJuIF90aGlzO1xuICB9XG5cbiAgLyoqXG4gICAqIFBhdGggb2YgcmVmZXJlbmNlZCBlbGVtZW50IHRvIHRyYW5zY2x1ZGUgaW5zdGVhZCBvZiBlbGVtZW50IGl0c2VsZi5cbiAgICogQHR5cGUgU3RyaW5nRWxlbWVudFxuICAgKiBAZGVmYXVsdCBlbGVtZW50XG4gICAqL1xuXG5cbiAgX2NyZWF0ZUNsYXNzKFJlZkVsZW1lbnQsIFt7XG4gICAga2V5OiAncGF0aCcsXG4gICAgZ2V0OiBmdW5jdGlvbiBnZXQoKSB7XG4gICAgICByZXR1cm4gdGhpcy5hdHRyaWJ1dGVzLmdldCgncGF0aCcpO1xuICAgIH0sXG4gICAgc2V0OiBmdW5jdGlvbiBzZXQobmV3VmFsdWUpIHtcbiAgICAgIHRoaXMuYXR0cmlidXRlcy5zZXQoJ3BhdGgnLCBuZXdWYWx1ZSk7XG4gICAgfVxuICB9XSk7XG5cbiAgcmV0dXJuIFJlZkVsZW1lbnQ7XG59KEVsZW1lbnQpOyIsIid1c2Ugc3RyaWN0JztcblxudmFyIE5hbWVzcGFjZSA9IHJlcXVpcmUoJy4vTmFtZXNwYWNlJyk7XG52YXIgZWxlbWVudHMgPSByZXF1aXJlKCcuL2VsZW1lbnRzJyk7XG5cbi8vIERpcmVjdCBhY2Nlc3MgdG8gdGhlIE5hbWVzcGFjZSBjbGFzc1xuZXhwb3J0cy5OYW1lc3BhY2UgPSBOYW1lc3BhY2U7XG5cbi8vIFNwZWNpYWwgY29uc3RydWN0b3IgZm9yIHRoZSBOYW1lc3BhY2UgY2xhc3NcbmV4cG9ydHMubmFtZXNwYWNlID0gZnVuY3Rpb24gbmFtZXNwYWNlKG9wdGlvbnMpIHtcbiAgcmV0dXJuIG5ldyBOYW1lc3BhY2Uob3B0aW9ucyk7XG59O1xuXG5leHBvcnRzLktleVZhbHVlUGFpciA9IHJlcXVpcmUoJy4vS2V5VmFsdWVQYWlyJyk7XG5cbmV4cG9ydHMuQXJyYXlTbGljZSA9IGVsZW1lbnRzLkFycmF5U2xpY2U7XG5leHBvcnRzLk9iamVjdFNsaWNlID0gZWxlbWVudHMuT2JqZWN0U2xpY2U7XG5cbmV4cG9ydHMuRWxlbWVudCA9IGVsZW1lbnRzLkVsZW1lbnQ7XG5leHBvcnRzLlN0cmluZ0VsZW1lbnQgPSBlbGVtZW50cy5TdHJpbmdFbGVtZW50O1xuZXhwb3J0cy5OdW1iZXJFbGVtZW50ID0gZWxlbWVudHMuTnVtYmVyRWxlbWVudDtcbmV4cG9ydHMuQm9vbGVhbkVsZW1lbnQgPSBlbGVtZW50cy5Cb29sZWFuRWxlbWVudDtcbmV4cG9ydHMuTnVsbEVsZW1lbnQgPSBlbGVtZW50cy5OdWxsRWxlbWVudDtcbmV4cG9ydHMuQXJyYXlFbGVtZW50ID0gZWxlbWVudHMuQXJyYXlFbGVtZW50O1xuZXhwb3J0cy5PYmplY3RFbGVtZW50ID0gZWxlbWVudHMuT2JqZWN0RWxlbWVudDtcbmV4cG9ydHMuTWVtYmVyRWxlbWVudCA9IGVsZW1lbnRzLk1lbWJlckVsZW1lbnQ7XG5leHBvcnRzLlJlZkVsZW1lbnQgPSBlbGVtZW50cy5SZWZFbGVtZW50O1xuZXhwb3J0cy5MaW5rRWxlbWVudCA9IGVsZW1lbnRzLkxpbmtFbGVtZW50O1xuXG5leHBvcnRzLnJlZnJhY3QgPSBlbGVtZW50cy5yZWZyYWN0O1xuXG5leHBvcnRzLkpTT05TZXJpYWxpc2VyID0gcmVxdWlyZSgnLi9zZXJpYWxpc2Vycy9KU09OU2VyaWFsaXNlcicpO1xuZXhwb3J0cy5KU09OMDZTZXJpYWxpc2VyID0gcmVxdWlyZSgnLi9zZXJpYWxpc2Vycy9KU09OMDZTZXJpYWxpc2VyJyk7IiwiJ3VzZSBzdHJpY3QnO1xuXG52YXIgX2NyZWF0ZUNsYXNzID0gZnVuY3Rpb24gKCkgeyBmdW5jdGlvbiBkZWZpbmVQcm9wZXJ0aWVzKHRhcmdldCwgcHJvcHMpIHsgZm9yICh2YXIgaSA9IDA7IGkgPCBwcm9wcy5sZW5ndGg7IGkrKykgeyB2YXIgZGVzY3JpcHRvciA9IHByb3BzW2ldOyBkZXNjcmlwdG9yLmVudW1lcmFibGUgPSBkZXNjcmlwdG9yLmVudW1lcmFibGUgfHwgZmFsc2U7IGRlc2NyaXB0b3IuY29uZmlndXJhYmxlID0gdHJ1ZTsgaWYgKFwidmFsdWVcIiBpbiBkZXNjcmlwdG9yKSBkZXNjcmlwdG9yLndyaXRhYmxlID0gdHJ1ZTsgT2JqZWN0LmRlZmluZVByb3BlcnR5KHRhcmdldCwgZGVzY3JpcHRvci5rZXksIGRlc2NyaXB0b3IpOyB9IH0gcmV0dXJuIGZ1bmN0aW9uIChDb25zdHJ1Y3RvciwgcHJvdG9Qcm9wcywgc3RhdGljUHJvcHMpIHsgaWYgKHByb3RvUHJvcHMpIGRlZmluZVByb3BlcnRpZXMoQ29uc3RydWN0b3IucHJvdG90eXBlLCBwcm90b1Byb3BzKTsgaWYgKHN0YXRpY1Byb3BzKSBkZWZpbmVQcm9wZXJ0aWVzKENvbnN0cnVjdG9yLCBzdGF0aWNQcm9wcyk7IHJldHVybiBDb25zdHJ1Y3RvcjsgfTsgfSgpO1xuXG5mdW5jdGlvbiBfY2xhc3NDYWxsQ2hlY2soaW5zdGFuY2UsIENvbnN0cnVjdG9yKSB7IGlmICghKGluc3RhbmNlIGluc3RhbmNlb2YgQ29uc3RydWN0b3IpKSB7IHRocm93IG5ldyBUeXBlRXJyb3IoXCJDYW5ub3QgY2FsbCBhIGNsYXNzIGFzIGEgZnVuY3Rpb25cIik7IH0gfVxuXG5mdW5jdGlvbiBfcG9zc2libGVDb25zdHJ1Y3RvclJldHVybihzZWxmLCBjYWxsKSB7IGlmICghc2VsZikgeyB0aHJvdyBuZXcgUmVmZXJlbmNlRXJyb3IoXCJ0aGlzIGhhc24ndCBiZWVuIGluaXRpYWxpc2VkIC0gc3VwZXIoKSBoYXNuJ3QgYmVlbiBjYWxsZWRcIik7IH0gcmV0dXJuIGNhbGwgJiYgKHR5cGVvZiBjYWxsID09PSBcIm9iamVjdFwiIHx8IHR5cGVvZiBjYWxsID09PSBcImZ1bmN0aW9uXCIpID8gY2FsbCA6IHNlbGY7IH1cblxuZnVuY3Rpb24gX2luaGVyaXRzKHN1YkNsYXNzLCBzdXBlckNsYXNzKSB7IGlmICh0eXBlb2Ygc3VwZXJDbGFzcyAhPT0gXCJmdW5jdGlvblwiICYmIHN1cGVyQ2xhc3MgIT09IG51bGwpIHsgdGhyb3cgbmV3IFR5cGVFcnJvcihcIlN1cGVyIGV4cHJlc3Npb24gbXVzdCBlaXRoZXIgYmUgbnVsbCBvciBhIGZ1bmN0aW9uLCBub3QgXCIgKyB0eXBlb2Ygc3VwZXJDbGFzcyk7IH0gc3ViQ2xhc3MucHJvdG90eXBlID0gT2JqZWN0LmNyZWF0ZShzdXBlckNsYXNzICYmIHN1cGVyQ2xhc3MucHJvdG90eXBlLCB7IGNvbnN0cnVjdG9yOiB7IHZhbHVlOiBzdWJDbGFzcywgZW51bWVyYWJsZTogZmFsc2UsIHdyaXRhYmxlOiB0cnVlLCBjb25maWd1cmFibGU6IHRydWUgfSB9KTsgaWYgKHN1cGVyQ2xhc3MpIE9iamVjdC5zZXRQcm90b3R5cGVPZiA/IE9iamVjdC5zZXRQcm90b3R5cGVPZihzdWJDbGFzcywgc3VwZXJDbGFzcykgOiBzdWJDbGFzcy5fX3Byb3RvX18gPSBzdXBlckNsYXNzOyB9XG5cbnZhciBuZWdhdGUgPSByZXF1aXJlKCdsb2Rhc2gvbmVnYXRlJyk7XG52YXIgRWxlbWVudCA9IHJlcXVpcmUoJy4vRWxlbWVudCcpO1xudmFyIEFycmF5U2xpY2UgPSByZXF1aXJlKCcuLi9BcnJheVNsaWNlJyk7XG5cbi8qKlxuICogQGNsYXNzXG4gKlxuICogQHBhcmFtIHtFbGVtZW50W119IGNvbnRlbnRcbiAqIEBwYXJhbSBtZXRhXG4gKiBAcGFyYW0gYXR0cmlidXRlc1xuICovXG5cbnZhciBBcnJheUVsZW1lbnQgPSBmdW5jdGlvbiAoX0VsZW1lbnQpIHtcbiAgX2luaGVyaXRzKEFycmF5RWxlbWVudCwgX0VsZW1lbnQpO1xuXG4gIGZ1bmN0aW9uIEFycmF5RWxlbWVudChjb250ZW50LCBtZXRhLCBhdHRyaWJ1dGVzKSB7XG4gICAgX2NsYXNzQ2FsbENoZWNrKHRoaXMsIEFycmF5RWxlbWVudCk7XG5cbiAgICB2YXIgX3RoaXMgPSBfcG9zc2libGVDb25zdHJ1Y3RvclJldHVybih0aGlzLCAoQXJyYXlFbGVtZW50Ll9fcHJvdG9fXyB8fCBPYmplY3QuZ2V0UHJvdG90eXBlT2YoQXJyYXlFbGVtZW50KSkuY2FsbCh0aGlzLCBjb250ZW50IHx8IFtdLCBtZXRhLCBhdHRyaWJ1dGVzKSk7XG5cbiAgICBfdGhpcy5lbGVtZW50ID0gJ2FycmF5JztcbiAgICByZXR1cm4gX3RoaXM7XG4gIH1cblxuICBfY3JlYXRlQ2xhc3MoQXJyYXlFbGVtZW50LCBbe1xuICAgIGtleTogJ3ByaW1pdGl2ZScsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIHByaW1pdGl2ZSgpIHtcbiAgICAgIHJldHVybiAnYXJyYXknO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEByZXR1cm5zIHtFbGVtZW50fVxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdnZXQnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBnZXQoaW5kZXgpIHtcbiAgICAgIHJldHVybiB0aGlzLmNvbnRlbnRbaW5kZXhdO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEhlbHBlciBmb3IgcmV0dXJuaW5nIHRoZSB2YWx1ZSBvZiBhbiBpdGVtXG4gICAgICogVGhpcyB3b3JrcyBmb3IgYm90aCBBcnJheUVsZW1lbnQgYW5kIE9iamVjdEVsZW1lbnQgaW5zdGFuY2VzXG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ2dldFZhbHVlJyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gZ2V0VmFsdWUoaW5kZXhPcktleSkge1xuICAgICAgdmFyIGl0ZW0gPSB0aGlzLmdldChpbmRleE9yS2V5KTtcblxuICAgICAgaWYgKGl0ZW0pIHtcbiAgICAgICAgcmV0dXJuIGl0ZW0udG9WYWx1ZSgpO1xuICAgICAgfVxuXG4gICAgICByZXR1cm4gdW5kZWZpbmVkO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEByZXR1cm5zIHtFbGVtZW50fVxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdnZXRJbmRleCcsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIGdldEluZGV4KGluZGV4KSB7XG4gICAgICByZXR1cm4gdGhpcy5jb250ZW50W2luZGV4XTtcbiAgICB9XG4gIH0sIHtcbiAgICBrZXk6ICdzZXQnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBzZXQoaW5kZXgsIHZhbHVlKSB7XG4gICAgICB0aGlzLmNvbnRlbnRbaW5kZXhdID0gdGhpcy5yZWZyYWN0KHZhbHVlKTtcbiAgICAgIHJldHVybiB0aGlzO1xuICAgIH1cbiAgfSwge1xuICAgIGtleTogJ3JlbW92ZScsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIHJlbW92ZShpbmRleCkge1xuICAgICAgdmFyIHJlbW92ZWQgPSB0aGlzLmNvbnRlbnQuc3BsaWNlKGluZGV4LCAxKTtcblxuICAgICAgaWYgKHJlbW92ZWQubGVuZ3RoKSB7XG4gICAgICAgIHJldHVybiByZW1vdmVkWzBdO1xuICAgICAgfVxuXG4gICAgICByZXR1cm4gbnVsbDtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBAcGFyYW0gY2FsbGJhY2sgLSBGdW5jdGlvbiB0byBleGVjdXRlIGZvciBlYWNoIGVsZW1lbnRcbiAgICAgKiBAcGFyYW0gdGhpc0FyZyAtIFZhbHVlIHRvIHVzZSBhcyB0aGlzIChpLmUgdGhlIHJlZmVyZW5jZSBPYmplY3QpIHdoZW4gZXhlY3V0aW5nIGNhbGxiYWNrXG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ21hcCcsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIG1hcChjYWxsYmFjaywgdGhpc0FyZykge1xuICAgICAgcmV0dXJuIHRoaXMuY29udGVudC5tYXAoY2FsbGJhY2ssIHRoaXNBcmcpO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIE1hcHMgYW5kIHRoZW4gZmxhdHRlbnMgdGhlIHJlc3VsdHMuXG4gICAgICogQHBhcmFtIGNhbGxiYWNrIC0gRnVuY3Rpb24gdG8gZXhlY3V0ZSBmb3IgZWFjaCBlbGVtZW50LlxuICAgICAqIEBwYXJhbSB0aGlzQXJnIC0gVmFsdWUgdG8gdXNlIGFzIHRoaXMgKGkuZSB0aGUgcmVmZXJlbmNlIE9iamVjdCkgd2hlbiBleGVjdXRpbmcgY2FsbGJhY2tcbiAgICAgKiBAcmV0dXJucyB7YXJyYXl9XG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ2ZsYXRNYXAnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBmbGF0TWFwKGNhbGxiYWNrLCB0aGlzQXJnKSB7XG4gICAgICByZXR1cm4gdGhpcy5tYXAoY2FsbGJhY2ssIHRoaXNBcmcpLnJlZHVjZShmdW5jdGlvbiAoYSwgYikge1xuICAgICAgICByZXR1cm4gYS5jb25jYXQoYik7XG4gICAgICB9LCBbXSk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogUmV0dXJucyBhbiBhcnJheSBjb250YWluaW5nIHRoZSB0cnV0aHkgcmVzdWx0cyBvZiBjYWxsaW5nIHRoZSBnaXZlbiB0cmFuc2Zvcm1hdGlvbiB3aXRoIGVhY2ggZWxlbWVudCBvZiB0aGlzIHNlcXVlbmNlXG4gICAgICogQHBhcmFtIHRyYW5zZm9ybSAtIEEgY2xvc3VyZSB0aGF0IGFjY2VwdHMgYW4gZWxlbWVudCBvZiB0aGlzIGFycmF5IGFzIGl0cyBhcmd1bWVudCBhbmQgcmV0dXJucyBhbiBvcHRpb25hbCB2YWx1ZS5cbiAgICAgKiBAcGFyYW0gdGhpc0FyZyAtIFZhbHVlIHRvIHVzZSBhcyB0aGlzIChpLmUgdGhlIHJlZmVyZW5jZSBPYmplY3QpIHdoZW4gZXhlY3V0aW5nIGNhbGxiYWNrXG4gICAgICogQG1lbWJlcm9mIEFycmF5RWxlbWVudC5wcm90b3R5cGVcbiAgICAgKiBAcmV0dXJucyBBbiBhcnJheSBvZiB0aGUgbm9uLXVuZGVmaW5lZCByZXN1bHRzIG9mIGNhbGxpbmcgdHJhbnNmb3JtIHdpdGggZWFjaCBlbGVtZW50IG9mIHRoZSBhcnJheVxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdjb21wYWN0TWFwJyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gY29tcGFjdE1hcCh0cmFuc2Zvcm0sIHRoaXNBcmcpIHtcbiAgICAgIHZhciByZXN1bHRzID0gW107XG5cbiAgICAgIHRoaXMuZm9yRWFjaChmdW5jdGlvbiAoZWxlbWVudCkge1xuICAgICAgICB2YXIgcmVzdWx0ID0gdHJhbnNmb3JtLmJpbmQodGhpc0FyZykoZWxlbWVudCk7XG5cbiAgICAgICAgaWYgKHJlc3VsdCkge1xuICAgICAgICAgIHJlc3VsdHMucHVzaChyZXN1bHQpO1xuICAgICAgICB9XG4gICAgICB9KTtcblxuICAgICAgcmV0dXJuIHJlc3VsdHM7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQHBhcmFtIGNhbGxiYWNrIC0gRnVuY3Rpb24gdG8gZXhlY3V0ZSBmb3IgZWFjaCBlbGVtZW50XG4gICAgICogQHBhcmFtIHRoaXNBcmcgLSBWYWx1ZSB0byB1c2UgYXMgdGhpcyAoaS5lIHRoZSByZWZlcmVuY2UgT2JqZWN0KSB3aGVuIGV4ZWN1dGluZyBjYWxsYmFja1xuICAgICAqIEByZXR1cm5zIHtBcnJheVNsaWNlfVxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdmaWx0ZXInLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBmaWx0ZXIoY2FsbGJhY2ssIHRoaXNBcmcpIHtcbiAgICAgIHJldHVybiBuZXcgQXJyYXlTbGljZSh0aGlzLmNvbnRlbnQuZmlsdGVyKGNhbGxiYWNrLCB0aGlzQXJnKSk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQHBhcmFtIGNhbGxiYWNrIC0gRnVuY3Rpb24gdG8gZXhlY3V0ZSBmb3IgZWFjaCBlbGVtZW50XG4gICAgICogQHBhcmFtIHRoaXNBcmcgLSBWYWx1ZSB0byB1c2UgYXMgdGhpcyAoaS5lIHRoZSByZWZlcmVuY2UgT2JqZWN0KSB3aGVuIGV4ZWN1dGluZyBjYWxsYmFja1xuICAgICAqIEByZXR1cm5zIHtBcnJheVNsaWNlfVxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdyZWplY3QnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiByZWplY3QoY2FsbGJhY2ssIHRoaXNBcmcpIHtcbiAgICAgIHJldHVybiB0aGlzLmZpbHRlcihuZWdhdGUoY2FsbGJhY2spLCB0aGlzQXJnKTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBUaGlzIGlzIGEgcmVkdWNlIGZ1bmN0aW9uIHNwZWNpZmljYWxseSBmb3IgTWluaW0gYXJyYXlzIGFuZCBvYmplY3RzLiBJdFxuICAgICAqIGFsbG93cyBmb3IgcmV0dXJuaW5nIG5vcm1hbCB2YWx1ZXMgb3IgTWluaW0gaW5zdGFuY2VzLCBzbyBpdCBjb252ZXJ0cyBhbnlcbiAgICAgKiBwcmltaXRpdmVzIG9uIGVhY2ggc3RlcC5cbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAncmVkdWNlJyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gcmVkdWNlKGNhbGxiYWNrLCBpbml0aWFsVmFsdWUpIHtcbiAgICAgIHZhciBzdGFydEluZGV4ID0gdm9pZCAwO1xuICAgICAgdmFyIG1lbW8gPSB2b2lkIDA7XG5cbiAgICAgIC8vIEFsbG93cyBmb3IgZGVmaW5pbmcgYSBzdGFydGluZyB2YWx1ZSBvZiB0aGUgcmVkdWNlXG4gICAgICBpZiAoaW5pdGlhbFZhbHVlICE9PSB1bmRlZmluZWQpIHtcbiAgICAgICAgc3RhcnRJbmRleCA9IDA7XG4gICAgICAgIG1lbW8gPSB0aGlzLnJlZnJhY3QoaW5pdGlhbFZhbHVlKTtcbiAgICAgIH0gZWxzZSB7XG4gICAgICAgIHN0YXJ0SW5kZXggPSAxO1xuICAgICAgICAvLyBPYmplY3QgRWxlbWVudCBjb250ZW50IGl0ZW1zIGFyZSBtZW1iZXIgZWxlbWVudHMuIEJlY2F1c2Ugb2YgdGhpcyxcbiAgICAgICAgLy8gdGhlIG1lbW8gc2hvdWxkIHN0YXJ0IG91dCBhcyB0aGUgbWVtYmVyIHZhbHVlIHJhdGhlciB0aGFuIHRoZVxuICAgICAgICAvLyBhY3R1YWwgbWVtYmVyIGl0c2VsZi5cbiAgICAgICAgbWVtbyA9IHRoaXMucHJpbWl0aXZlKCkgPT09ICdvYmplY3QnID8gdGhpcy5maXJzdC52YWx1ZSA6IHRoaXMuZmlyc3Q7XG4gICAgICB9XG5cbiAgICAgIC8vIFNlbmRpbmcgZWFjaCBmdW5jdGlvbiBjYWxsIHRvIHRoZSByZWdpc3RyeSBhbGxvd3MgZm9yIHBhc3NpbmcgTWluaW1cbiAgICAgIC8vIGluc3RhbmNlcyB0aHJvdWdoIHRoZSBmdW5jdGlvbiByZXR1cm4uIFRoaXMgbWVhbnMgeW91IGNhbiByZXR1cm5cbiAgICAgIC8vIHByaW1pdGl2ZSB2YWx1ZXMgb3IgcmV0dXJuIE1pbmltIGluc3RhbmNlcyBhbmQgcmVkdWNlIHdpbGwgc3RpbGwgd29yay5cbiAgICAgIGZvciAodmFyIGkgPSBzdGFydEluZGV4OyBpIDwgdGhpcy5sZW5ndGg7IGkgKz0gMSkge1xuICAgICAgICB2YXIgaXRlbSA9IHRoaXMuY29udGVudFtpXTtcblxuICAgICAgICBpZiAodGhpcy5wcmltaXRpdmUoKSA9PT0gJ29iamVjdCcpIHtcbiAgICAgICAgICBtZW1vID0gdGhpcy5yZWZyYWN0KGNhbGxiYWNrKG1lbW8sIGl0ZW0udmFsdWUsIGl0ZW0ua2V5LCBpdGVtLCB0aGlzKSk7XG4gICAgICAgIH0gZWxzZSB7XG4gICAgICAgICAgbWVtbyA9IHRoaXMucmVmcmFjdChjYWxsYmFjayhtZW1vLCBpdGVtLCBpLCB0aGlzKSk7XG4gICAgICAgIH1cbiAgICAgIH1cblxuICAgICAgcmV0dXJuIG1lbW87XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQGNhbGxiYWNrIGZvckVhY2hDYWxsYmFja1xuICAgICAqIEBwYXJhbSB7RWxlbWVudH0gY3VycmVudFZhbHVlXG4gICAgICogQHBhcmFtIHtOdW1iZXJFbGVtZW50fSBpbmRleFxuICAgICAqL1xuXG4gICAgLyoqXG4gICAgICogQHBhcmFtIHtmb3JFYWNoQ2FsbGJhY2t9IGNhbGxiYWNrIC0gRnVuY3Rpb24gdG8gZXhlY3V0ZSBmb3IgZWFjaCBlbGVtZW50XG4gICAgICogQHBhcmFtIHRoaXNBcmcgLSBWYWx1ZSB0byB1c2UgYXMgdGhpcyAoaS5lIHRoZSByZWZlcmVuY2UgT2JqZWN0KSB3aGVuIGV4ZWN1dGluZyBjYWxsYmFja1xuICAgICAqIEBtZW1iZXJvZiBBcnJheUVsZW1lbnQucHJvdG90eXBlXG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ2ZvckVhY2gnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBmb3JFYWNoKGNhbGxiYWNrLCB0aGlzQXJnKSB7XG4gICAgICB2YXIgX3RoaXMyID0gdGhpcztcblxuICAgICAgdGhpcy5jb250ZW50LmZvckVhY2goZnVuY3Rpb24gKGl0ZW0sIGluZGV4KSB7XG4gICAgICAgIGNhbGxiYWNrLmJpbmQodGhpc0FyZykoaXRlbSwgX3RoaXMyLnJlZnJhY3QoaW5kZXgpKTtcbiAgICAgIH0pO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEByZXR1cm5zIHtFbGVtZW50fVxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdzaGlmdCcsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIHNoaWZ0KCkge1xuICAgICAgcmV0dXJuIHRoaXMuY29udGVudC5zaGlmdCgpO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEBwYXJhbSB2YWx1ZVxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICd1bnNoaWZ0JyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gdW5zaGlmdCh2YWx1ZSkge1xuICAgICAgdGhpcy5jb250ZW50LnVuc2hpZnQodGhpcy5yZWZyYWN0KHZhbHVlKSk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQHBhcmFtIHZhbHVlXG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ3B1c2gnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBwdXNoKHZhbHVlKSB7XG4gICAgICB0aGlzLmNvbnRlbnQucHVzaCh0aGlzLnJlZnJhY3QodmFsdWUpKTtcbiAgICAgIHJldHVybiB0aGlzO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEBwYXJhbSB2YWx1ZVxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdhZGQnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBhZGQodmFsdWUpIHtcbiAgICAgIHRoaXMucHVzaCh2YWx1ZSk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogUmVjdXNpdmVseSBzZWFyY2ggYWxsIGRlc2NlbmRlbnRzIHVzaW5nIGEgY29uZGl0aW9uIGZ1bmN0aW9uLlxuICAgICAqIEByZXR1cm5zIHtFbGVtZW50W119XG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ2ZpbmRFbGVtZW50cycsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIGZpbmRFbGVtZW50cyhjb25kaXRpb24sIGdpdmVuT3B0aW9ucykge1xuICAgICAgdmFyIG9wdGlvbnMgPSBnaXZlbk9wdGlvbnMgfHwge307XG4gICAgICB2YXIgcmVjdXJzaXZlID0gISFvcHRpb25zLnJlY3Vyc2l2ZTtcbiAgICAgIHZhciByZXN1bHRzID0gb3B0aW9ucy5yZXN1bHRzID09PSB1bmRlZmluZWQgPyBbXSA6IG9wdGlvbnMucmVzdWx0cztcblxuICAgICAgLy8gVGhlIGZvckVhY2ggbWV0aG9kIGZvciBPYmplY3QgRWxlbWVudHMgcmV0dXJucyB2YWx1ZSwga2V5LCBhbmQgbWVtYmVyLlxuICAgICAgLy8gVGhpcyBwYXNzZXMgdGhvc2UgYWxvbmcgdG8gdGhlIGNvbmRpdGlvbiBmdW5jdGlvbiBiZWxvdy5cbiAgICAgIHRoaXMuZm9yRWFjaChmdW5jdGlvbiAoaXRlbSwga2V5T3JJbmRleCwgbWVtYmVyKSB7XG4gICAgICAgIC8vIFdlIHVzZSBkdWNrLXR5cGluZyBoZXJlIHRvIHN1cHBvcnQgYW55IHJlZ2lzdGVyZWQgY2xhc3MgdGhhdFxuICAgICAgICAvLyBtYXkgY29udGFpbiBvdGhlciBlbGVtZW50cy5cbiAgICAgICAgaWYgKHJlY3Vyc2l2ZSAmJiBpdGVtLmZpbmRFbGVtZW50cyAhPT0gdW5kZWZpbmVkKSB7XG4gICAgICAgICAgaXRlbS5maW5kRWxlbWVudHMoY29uZGl0aW9uLCB7XG4gICAgICAgICAgICByZXN1bHRzOiByZXN1bHRzLFxuICAgICAgICAgICAgcmVjdXJzaXZlOiByZWN1cnNpdmVcbiAgICAgICAgICB9KTtcbiAgICAgICAgfVxuXG4gICAgICAgIGlmIChjb25kaXRpb24oaXRlbSwga2V5T3JJbmRleCwgbWVtYmVyKSkge1xuICAgICAgICAgIHJlc3VsdHMucHVzaChpdGVtKTtcbiAgICAgICAgfVxuICAgICAgfSk7XG5cbiAgICAgIHJldHVybiByZXN1bHRzO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIFJlY3VzaXZlbHkgc2VhcmNoIGFsbCBkZXNjZW5kZW50cyB1c2luZyBhIGNvbmRpdGlvbiBmdW5jdGlvbi5cbiAgICAgKiBAcGFyYW0gY29uZGl0aW9uXG4gICAgICogQHJldHVybnMge0FycmF5U2xpY2V9XG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ2ZpbmQnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBmaW5kKGNvbmRpdGlvbikge1xuICAgICAgcmV0dXJuIG5ldyBBcnJheVNsaWNlKHRoaXMuZmluZEVsZW1lbnRzKGNvbmRpdGlvbiwgeyByZWN1cnNpdmU6IHRydWUgfSkpO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEBwYXJhbSB7c3RyaW5nfSBlbGVtZW50XG4gICAgICogQHJldHVybnMge0FycmF5U2xpY2V9XG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ2ZpbmRCeUVsZW1lbnQnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBmaW5kQnlFbGVtZW50KGVsZW1lbnQpIHtcbiAgICAgIHJldHVybiB0aGlzLmZpbmQoZnVuY3Rpb24gKGl0ZW0pIHtcbiAgICAgICAgcmV0dXJuIGl0ZW0uZWxlbWVudCA9PT0gZWxlbWVudDtcbiAgICAgIH0pO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEBwYXJhbSB7c3RyaW5nfSBjbGFzc05hbWVcbiAgICAgKiBAcmV0dXJucyB7QXJyYXlTbGljZX1cbiAgICAgKiBAbWVtYmVyb2YgQXJyYXlFbGVtZW50LnByb3RvdHlwZVxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdmaW5kQnlDbGFzcycsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIGZpbmRCeUNsYXNzKGNsYXNzTmFtZSkge1xuICAgICAgcmV0dXJuIHRoaXMuZmluZChmdW5jdGlvbiAoaXRlbSkge1xuICAgICAgICByZXR1cm4gaXRlbS5jbGFzc2VzLmluY2x1ZGVzKGNsYXNzTmFtZSk7XG4gICAgICB9KTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBTZWFyY2ggdGhlIHRyZWUgcmVjdXJzaXZlbHkgYW5kIGZpbmQgdGhlIGVsZW1lbnQgd2l0aCB0aGUgbWF0Y2hpbmcgSURcbiAgICAgKiBAcGFyYW0ge3N0cmluZ30gaWRcbiAgICAgKiBAcmV0dXJucyB7RWxlbWVudH1cbiAgICAgKiBAbWVtYmVyb2YgQXJyYXlFbGVtZW50LnByb3RvdHlwZVxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdnZXRCeUlkJyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gZ2V0QnlJZChpZCkge1xuICAgICAgcmV0dXJuIHRoaXMuZmluZChmdW5jdGlvbiAoaXRlbSkge1xuICAgICAgICByZXR1cm4gaXRlbS5pZC50b1ZhbHVlKCkgPT09IGlkO1xuICAgICAgfSkuZmlyc3Q7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogTG9va3MgZm9yIG1hdGNoaW5nIGNoaWxkcmVuIHVzaW5nIGRlZXAgZXF1YWxpdHlcbiAgICAgKiBAcGFyYW0gdmFsdWVcbiAgICAgKiBAcmV0dXJucyB7Ym9vbGVhbn1cbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAnaW5jbHVkZXMnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBpbmNsdWRlcyh2YWx1ZSkge1xuICAgICAgcmV0dXJuIHRoaXMuY29udGVudC5zb21lKGZ1bmN0aW9uIChlbGVtZW50KSB7XG4gICAgICAgIHJldHVybiBlbGVtZW50LmVxdWFscyh2YWx1ZSk7XG4gICAgICB9KTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBMb29rcyBmb3IgbWF0Y2hpbmcgY2hpbGRyZW4gdXNpbmcgZGVlcCBlcXVhbGl0eVxuICAgICAqIEBwYXJhbSB2YWx1ZVxuICAgICAqIEByZXR1cm5zIHtib29sZWFufVxuICAgICAqIEBzZWUgaW5jbHVkZXNcbiAgICAgKiBAZGVwcmVjYXRlZCBtZXRob2Qgd2FzIHJlcGxhY2VkIGJ5IGluY2x1ZGVzXG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ2NvbnRhaW5zJyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gY29udGFpbnModmFsdWUpIHtcbiAgICAgIHJldHVybiB0aGlzLmluY2x1ZGVzKHZhbHVlKTtcbiAgICB9XG5cbiAgICAvLyBGYW50YXN5IExhbmRcblxuICAgIC8qKlxuICAgICAqIEByZXR1cm5zIHtBcnJheUVsZW1lbnR9IEFuIGVtcHR5IGFycmF5IGVsZW1lbnRcbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAnZW1wdHknLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBlbXB0eSgpIHtcbiAgICAgIHJldHVybiBuZXcgdGhpcy5jb25zdHJ1Y3RvcihbXSk7XG4gICAgfVxuICB9LCB7XG4gICAga2V5OiAnZmFudGFzeS1sYW5kL2VtcHR5JyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gZmFudGFzeUxhbmRFbXB0eSgpIHtcbiAgICAgIHJldHVybiB0aGlzLmVtcHR5KCk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQHBhcmFtIHtBcnJheUVsZW1lbnR9IG90aGVyXG4gICAgICogQHJldHVybnMge0FycmF5RWxlbWVudH1cbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAnY29uY2F0JyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gY29uY2F0KG90aGVyKSB7XG4gICAgICByZXR1cm4gbmV3IHRoaXMuY29uc3RydWN0b3IodGhpcy5jb250ZW50LmNvbmNhdChvdGhlci5jb250ZW50KSk7XG4gICAgfVxuICB9LCB7XG4gICAga2V5OiAnZmFudGFzeS1sYW5kL2NvbmNhdCcsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIGZhbnRhc3lMYW5kQ29uY2F0KG90aGVyKSB7XG4gICAgICByZXR1cm4gdGhpcy5jb25jYXQob3RoZXIpO1xuICAgIH1cbiAgfSwge1xuICAgIGtleTogJ2ZhbnRhc3ktbGFuZC9tYXAnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBmYW50YXN5TGFuZE1hcCh0cmFuc2Zvcm0pIHtcbiAgICAgIHJldHVybiBuZXcgdGhpcy5jb25zdHJ1Y3Rvcih0aGlzLm1hcCh0cmFuc2Zvcm0pKTtcbiAgICB9XG4gIH0sIHtcbiAgICBrZXk6ICdmYW50YXN5LWxhbmQvY2hhaW4nLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBmYW50YXN5TGFuZENoYWluKHRyYW5zZm9ybSkge1xuICAgICAgcmV0dXJuIHRoaXMubWFwKGZ1bmN0aW9uIChlbGVtZW50KSB7XG4gICAgICAgIHJldHVybiB0cmFuc2Zvcm0oZWxlbWVudCk7XG4gICAgICB9LCB0aGlzKS5yZWR1Y2UoZnVuY3Rpb24gKGEsIGIpIHtcbiAgICAgICAgcmV0dXJuIGEuY29uY2F0KGIpO1xuICAgICAgfSwgdGhpcy5lbXB0eSgpKTtcbiAgICB9XG4gIH0sIHtcbiAgICBrZXk6ICdmYW50YXN5LWxhbmQvZmlsdGVyJyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gZmFudGFzeUxhbmRGaWx0ZXIoY2FsbGJhY2spIHtcbiAgICAgIHJldHVybiBuZXcgdGhpcy5jb25zdHJ1Y3Rvcih0aGlzLmNvbnRlbnQuZmlsdGVyKGNhbGxiYWNrKSk7XG4gICAgfVxuICB9LCB7XG4gICAga2V5OiAnZmFudGFzeS1sYW5kL3JlZHVjZScsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIGZhbnRhc3lMYW5kUmVkdWNlKHRyYW5zZm9ybSwgaW5pdGlhbFZhbHVlKSB7XG4gICAgICByZXR1cm4gdGhpcy5jb250ZW50LnJlZHVjZSh0cmFuc2Zvcm0sIGluaXRpYWxWYWx1ZSk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogUmV0dXJucyB0aGUgbGVuZ3RoIG9mIHRoZSBjb2xsZWN0aW9uXG4gICAgICogQHR5cGUgbnVtYmVyXG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ2xlbmd0aCcsXG4gICAgZ2V0OiBmdW5jdGlvbiBnZXQoKSB7XG4gICAgICByZXR1cm4gdGhpcy5jb250ZW50Lmxlbmd0aDtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBSZXR1cm5zIHdoZXRoZXIgdGhlIGNvbGxlY3Rpb24gaXMgZW1wdHlcbiAgICAgKiBAdHlwZSBib29sZWFuXG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ2lzRW1wdHknLFxuICAgIGdldDogZnVuY3Rpb24gZ2V0KCkge1xuICAgICAgcmV0dXJuIHRoaXMuY29udGVudC5sZW5ndGggPT09IDA7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogUmV0dXJuIHRoZSBmaXJzdCBpdGVtIGluIHRoZSBjb2xsZWN0aW9uXG4gICAgICogQHR5cGUgRWxlbWVudFxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdmaXJzdCcsXG4gICAgZ2V0OiBmdW5jdGlvbiBnZXQoKSB7XG4gICAgICByZXR1cm4gdGhpcy5nZXRJbmRleCgwKTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBSZXR1cm4gdGhlIHNlY29uZCBpdGVtIGluIHRoZSBjb2xsZWN0aW9uXG4gICAgICogQHR5cGUgRWxlbWVudFxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdzZWNvbmQnLFxuICAgIGdldDogZnVuY3Rpb24gZ2V0KCkge1xuICAgICAgcmV0dXJuIHRoaXMuZ2V0SW5kZXgoMSk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogUmV0dXJuIHRoZSBsYXN0IGl0ZW0gaW4gdGhlIGNvbGxlY3Rpb25cbiAgICAgKiBAdHlwZSBFbGVtZW50XG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ2xhc3QnLFxuICAgIGdldDogZnVuY3Rpb24gZ2V0KCkge1xuICAgICAgcmV0dXJuIHRoaXMuZ2V0SW5kZXgodGhpcy5sZW5ndGggLSAxKTtcbiAgICB9XG4gIH1dKTtcblxuICByZXR1cm4gQXJyYXlFbGVtZW50O1xufShFbGVtZW50KTtcblxuLyoqXG4gKiBAcmV0dXJucyB7QXJyYXlFbGVtZW50fSBBbiBlbXB0eSBhcnJheSBlbGVtZW50XG4gKi9cblxuXG5BcnJheUVsZW1lbnQuZW1wdHkgPSBmdW5jdGlvbiBlbXB0eSgpIHtcbiAgcmV0dXJuIG5ldyB0aGlzKCk7XG59O1xuXG5BcnJheUVsZW1lbnRbJ2ZhbnRhc3ktbGFuZC9lbXB0eSddID0gQXJyYXlFbGVtZW50LmVtcHR5O1xuXG5pZiAodHlwZW9mIFN5bWJvbCAhPT0gJ3VuZGVmaW5lZCcpIHtcbiAgQXJyYXlFbGVtZW50LnByb3RvdHlwZVtTeW1ib2wuaXRlcmF0b3JdID0gZnVuY3Rpb24gc3ltYm9sKCkge1xuICAgIHJldHVybiB0aGlzLmNvbnRlbnRbU3ltYm9sLml0ZXJhdG9yXSgpO1xuICB9O1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IEFycmF5RWxlbWVudDsiLCIndXNlIHN0cmljdCc7XG5cbnZhciBfY3JlYXRlQ2xhc3MgPSBmdW5jdGlvbiAoKSB7IGZ1bmN0aW9uIGRlZmluZVByb3BlcnRpZXModGFyZ2V0LCBwcm9wcykgeyBmb3IgKHZhciBpID0gMDsgaSA8IHByb3BzLmxlbmd0aDsgaSsrKSB7IHZhciBkZXNjcmlwdG9yID0gcHJvcHNbaV07IGRlc2NyaXB0b3IuZW51bWVyYWJsZSA9IGRlc2NyaXB0b3IuZW51bWVyYWJsZSB8fCBmYWxzZTsgZGVzY3JpcHRvci5jb25maWd1cmFibGUgPSB0cnVlOyBpZiAoXCJ2YWx1ZVwiIGluIGRlc2NyaXB0b3IpIGRlc2NyaXB0b3Iud3JpdGFibGUgPSB0cnVlOyBPYmplY3QuZGVmaW5lUHJvcGVydHkodGFyZ2V0LCBkZXNjcmlwdG9yLmtleSwgZGVzY3JpcHRvcik7IH0gfSByZXR1cm4gZnVuY3Rpb24gKENvbnN0cnVjdG9yLCBwcm90b1Byb3BzLCBzdGF0aWNQcm9wcykgeyBpZiAocHJvdG9Qcm9wcykgZGVmaW5lUHJvcGVydGllcyhDb25zdHJ1Y3Rvci5wcm90b3R5cGUsIHByb3RvUHJvcHMpOyBpZiAoc3RhdGljUHJvcHMpIGRlZmluZVByb3BlcnRpZXMoQ29uc3RydWN0b3IsIHN0YXRpY1Byb3BzKTsgcmV0dXJuIENvbnN0cnVjdG9yOyB9OyB9KCk7XG5cbmZ1bmN0aW9uIF9jbGFzc0NhbGxDaGVjayhpbnN0YW5jZSwgQ29uc3RydWN0b3IpIHsgaWYgKCEoaW5zdGFuY2UgaW5zdGFuY2VvZiBDb25zdHJ1Y3RvcikpIHsgdGhyb3cgbmV3IFR5cGVFcnJvcihcIkNhbm5vdCBjYWxsIGEgY2xhc3MgYXMgYSBmdW5jdGlvblwiKTsgfSB9XG5cbmZ1bmN0aW9uIF9wb3NzaWJsZUNvbnN0cnVjdG9yUmV0dXJuKHNlbGYsIGNhbGwpIHsgaWYgKCFzZWxmKSB7IHRocm93IG5ldyBSZWZlcmVuY2VFcnJvcihcInRoaXMgaGFzbid0IGJlZW4gaW5pdGlhbGlzZWQgLSBzdXBlcigpIGhhc24ndCBiZWVuIGNhbGxlZFwiKTsgfSByZXR1cm4gY2FsbCAmJiAodHlwZW9mIGNhbGwgPT09IFwib2JqZWN0XCIgfHwgdHlwZW9mIGNhbGwgPT09IFwiZnVuY3Rpb25cIikgPyBjYWxsIDogc2VsZjsgfVxuXG5mdW5jdGlvbiBfaW5oZXJpdHMoc3ViQ2xhc3MsIHN1cGVyQ2xhc3MpIHsgaWYgKHR5cGVvZiBzdXBlckNsYXNzICE9PSBcImZ1bmN0aW9uXCIgJiYgc3VwZXJDbGFzcyAhPT0gbnVsbCkgeyB0aHJvdyBuZXcgVHlwZUVycm9yKFwiU3VwZXIgZXhwcmVzc2lvbiBtdXN0IGVpdGhlciBiZSBudWxsIG9yIGEgZnVuY3Rpb24sIG5vdCBcIiArIHR5cGVvZiBzdXBlckNsYXNzKTsgfSBzdWJDbGFzcy5wcm90b3R5cGUgPSBPYmplY3QuY3JlYXRlKHN1cGVyQ2xhc3MgJiYgc3VwZXJDbGFzcy5wcm90b3R5cGUsIHsgY29uc3RydWN0b3I6IHsgdmFsdWU6IHN1YkNsYXNzLCBlbnVtZXJhYmxlOiBmYWxzZSwgd3JpdGFibGU6IHRydWUsIGNvbmZpZ3VyYWJsZTogdHJ1ZSB9IH0pOyBpZiAoc3VwZXJDbGFzcykgT2JqZWN0LnNldFByb3RvdHlwZU9mID8gT2JqZWN0LnNldFByb3RvdHlwZU9mKHN1YkNsYXNzLCBzdXBlckNsYXNzKSA6IHN1YkNsYXNzLl9fcHJvdG9fXyA9IHN1cGVyQ2xhc3M7IH1cblxudmFyIEVsZW1lbnQgPSByZXF1aXJlKCcuL0VsZW1lbnQnKTtcblxuLyoqXG4gKiBAY2xhc3MgQm9vbGVhbkVsZW1lbnRcbiAqXG4gKiBAcGFyYW0ge2Jvb2xlYW59IGNvbnRlbnRcbiAqIEBwYXJhbSBtZXRhXG4gKiBAcGFyYW0gYXR0cmlidXRlc1xuICovXG5tb2R1bGUuZXhwb3J0cyA9IGZ1bmN0aW9uIChfRWxlbWVudCkge1xuICBfaW5oZXJpdHMoQm9vbGVhbkVsZW1lbnQsIF9FbGVtZW50KTtcblxuICBmdW5jdGlvbiBCb29sZWFuRWxlbWVudChjb250ZW50LCBtZXRhLCBhdHRyaWJ1dGVzKSB7XG4gICAgX2NsYXNzQ2FsbENoZWNrKHRoaXMsIEJvb2xlYW5FbGVtZW50KTtcblxuICAgIHZhciBfdGhpcyA9IF9wb3NzaWJsZUNvbnN0cnVjdG9yUmV0dXJuKHRoaXMsIChCb29sZWFuRWxlbWVudC5fX3Byb3RvX18gfHwgT2JqZWN0LmdldFByb3RvdHlwZU9mKEJvb2xlYW5FbGVtZW50KSkuY2FsbCh0aGlzLCBjb250ZW50LCBtZXRhLCBhdHRyaWJ1dGVzKSk7XG5cbiAgICBfdGhpcy5lbGVtZW50ID0gJ2Jvb2xlYW4nO1xuICAgIHJldHVybiBfdGhpcztcbiAgfVxuXG4gIF9jcmVhdGVDbGFzcyhCb29sZWFuRWxlbWVudCwgW3tcbiAgICBrZXk6ICdwcmltaXRpdmUnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBwcmltaXRpdmUoKSB7XG4gICAgICByZXR1cm4gJ2Jvb2xlYW4nO1xuICAgIH1cbiAgfV0pO1xuXG4gIHJldHVybiBCb29sZWFuRWxlbWVudDtcbn0oRWxlbWVudCk7IiwiJ3VzZSBzdHJpY3QnO1xuXG52YXIgX3R5cGVvZiA9IHR5cGVvZiBTeW1ib2wgPT09IFwiZnVuY3Rpb25cIiAmJiB0eXBlb2YgU3ltYm9sLml0ZXJhdG9yID09PSBcInN5bWJvbFwiID8gZnVuY3Rpb24gKG9iaikgeyByZXR1cm4gdHlwZW9mIG9iajsgfSA6IGZ1bmN0aW9uIChvYmopIHsgcmV0dXJuIG9iaiAmJiB0eXBlb2YgU3ltYm9sID09PSBcImZ1bmN0aW9uXCIgJiYgb2JqLmNvbnN0cnVjdG9yID09PSBTeW1ib2wgJiYgb2JqICE9PSBTeW1ib2wucHJvdG90eXBlID8gXCJzeW1ib2xcIiA6IHR5cGVvZiBvYmo7IH07XG5cbnZhciBfY3JlYXRlQ2xhc3MgPSBmdW5jdGlvbiAoKSB7IGZ1bmN0aW9uIGRlZmluZVByb3BlcnRpZXModGFyZ2V0LCBwcm9wcykgeyBmb3IgKHZhciBpID0gMDsgaSA8IHByb3BzLmxlbmd0aDsgaSsrKSB7IHZhciBkZXNjcmlwdG9yID0gcHJvcHNbaV07IGRlc2NyaXB0b3IuZW51bWVyYWJsZSA9IGRlc2NyaXB0b3IuZW51bWVyYWJsZSB8fCBmYWxzZTsgZGVzY3JpcHRvci5jb25maWd1cmFibGUgPSB0cnVlOyBpZiAoXCJ2YWx1ZVwiIGluIGRlc2NyaXB0b3IpIGRlc2NyaXB0b3Iud3JpdGFibGUgPSB0cnVlOyBPYmplY3QuZGVmaW5lUHJvcGVydHkodGFyZ2V0LCBkZXNjcmlwdG9yLmtleSwgZGVzY3JpcHRvcik7IH0gfSByZXR1cm4gZnVuY3Rpb24gKENvbnN0cnVjdG9yLCBwcm90b1Byb3BzLCBzdGF0aWNQcm9wcykgeyBpZiAocHJvdG9Qcm9wcykgZGVmaW5lUHJvcGVydGllcyhDb25zdHJ1Y3Rvci5wcm90b3R5cGUsIHByb3RvUHJvcHMpOyBpZiAoc3RhdGljUHJvcHMpIGRlZmluZVByb3BlcnRpZXMoQ29uc3RydWN0b3IsIHN0YXRpY1Byb3BzKTsgcmV0dXJuIENvbnN0cnVjdG9yOyB9OyB9KCk7XG5cbmZ1bmN0aW9uIF9jbGFzc0NhbGxDaGVjayhpbnN0YW5jZSwgQ29uc3RydWN0b3IpIHsgaWYgKCEoaW5zdGFuY2UgaW5zdGFuY2VvZiBDb25zdHJ1Y3RvcikpIHsgdGhyb3cgbmV3IFR5cGVFcnJvcihcIkNhbm5vdCBjYWxsIGEgY2xhc3MgYXMgYSBmdW5jdGlvblwiKTsgfSB9XG5cbnZhciBpc0VxdWFsID0gcmVxdWlyZSgnbG9kYXNoL2lzRXF1YWwnKTtcbnZhciBLZXlWYWx1ZVBhaXIgPSByZXF1aXJlKCcuLi9LZXlWYWx1ZVBhaXInKTtcbnZhciBBcnJheVNsaWNlID0gcmVxdWlyZSgnLi4vQXJyYXlTbGljZS5qcycpO1xuXG4vKipcbiAqIEBjbGFzc1xuICpcbiAqIEBwYXJhbSBjb250ZW50XG4gKiBAcGFyYW0gbWV0YVxuICogQHBhcmFtIGF0dHJpYnV0ZXNcbiAqXG4gKiBAcHJvcGVydHkge3N0cmluZ30gZWxlbWVudFxuICovXG5cbnZhciBFbGVtZW50ID0gZnVuY3Rpb24gKCkge1xuICBmdW5jdGlvbiBFbGVtZW50KGNvbnRlbnQsIG1ldGEsIGF0dHJpYnV0ZXMpIHtcbiAgICBfY2xhc3NDYWxsQ2hlY2sodGhpcywgRWxlbWVudCk7XG5cbiAgICAvLyBMYXp5IGxvYWQgdGhpcy5tZXRhIGFuZCB0aGlzLmF0dHJpYnV0ZXMgYmVjYXVzZSBpdCdzIGEgTWluaW0gZWxlbWVudFxuICAgIC8vIE90aGVyd2lzZSwgd2UgZ2V0IGludG8gY2lyY3VsdWFyIGNhbGxzXG4gICAgaWYgKG1ldGEpIHtcbiAgICAgIHRoaXMubWV0YSA9IG1ldGE7XG4gICAgfVxuXG4gICAgaWYgKGF0dHJpYnV0ZXMpIHtcbiAgICAgIHRoaXMuYXR0cmlidXRlcyA9IGF0dHJpYnV0ZXM7XG4gICAgfVxuXG4gICAgdGhpcy5jb250ZW50ID0gY29udGVudDtcbiAgfVxuXG4gIC8qKlxuICAgKiBGcmVlemVzIHRoZSBlbGVtZW50IHRvIHByZXZlbnQgYW55IG11dGF0aW9uLlxuICAgKiBBIGZyb3plbiBlbGVtZW50IHdpbGwgYWRkIGBwYXJlbnRgIHByb3BlcnR5IHRvIGV2ZXJ5IGNoaWxkIGVsZW1lbnRcbiAgICogdG8gYWxsb3cgdHJhdmVyc2luZyB1cCB0aGUgZWxlbWVudCB0cmVlLlxuICAgKi9cblxuXG4gIF9jcmVhdGVDbGFzcyhFbGVtZW50LCBbe1xuICAgIGtleTogJ2ZyZWV6ZScsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIGZyZWV6ZSgpIHtcbiAgICAgIHZhciBfdGhpcyA9IHRoaXM7XG5cbiAgICAgIGlmIChPYmplY3QuaXNGcm96ZW4odGhpcykpIHtcbiAgICAgICAgcmV0dXJuO1xuICAgICAgfVxuXG4gICAgICBpZiAodGhpcy5fbWV0YSkge1xuICAgICAgICB0aGlzLm1ldGEucGFyZW50ID0gdGhpcztcbiAgICAgICAgdGhpcy5tZXRhLmZyZWV6ZSgpO1xuICAgICAgfVxuXG4gICAgICBpZiAodGhpcy5fYXR0cmlidXRlcykge1xuICAgICAgICB0aGlzLmF0dHJpYnV0ZXMucGFyZW50ID0gdGhpcztcbiAgICAgICAgdGhpcy5hdHRyaWJ1dGVzLmZyZWV6ZSgpO1xuICAgICAgfVxuXG4gICAgICB0aGlzLmNoaWxkcmVuLmZvckVhY2goZnVuY3Rpb24gKGVsZW1lbnQpIHtcbiAgICAgICAgZWxlbWVudC5wYXJlbnQgPSBfdGhpcztcbiAgICAgICAgZWxlbWVudC5mcmVlemUoKTtcbiAgICAgIH0sIHRoaXMpO1xuXG4gICAgICBpZiAodGhpcy5jb250ZW50ICYmIEFycmF5LmlzQXJyYXkodGhpcy5jb250ZW50KSkge1xuICAgICAgICBPYmplY3QuZnJlZXplKHRoaXMuY29udGVudCk7XG4gICAgICB9XG5cbiAgICAgIE9iamVjdC5mcmVlemUodGhpcyk7XG4gICAgfVxuICB9LCB7XG4gICAga2V5OiAncHJpbWl0aXZlJyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gcHJpbWl0aXZlKCkge31cblxuICAgIC8qKlxuICAgICAqIENyZWF0ZXMgYSBkZWVwIGNsb25lIG9mIHRoZSBpbnN0YW5jZVxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdjbG9uZScsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIGNsb25lKCkge1xuICAgICAgdmFyIGNvcHkgPSBuZXcgdGhpcy5jb25zdHJ1Y3RvcigpO1xuXG4gICAgICBjb3B5LmVsZW1lbnQgPSB0aGlzLmVsZW1lbnQ7XG5cbiAgICAgIGlmICh0aGlzLm1ldGEubGVuZ3RoKSB7XG4gICAgICAgIGNvcHkuX21ldGEgPSB0aGlzLm1ldGEuY2xvbmUoKTtcbiAgICAgIH1cblxuICAgICAgaWYgKHRoaXMuYXR0cmlidXRlcy5sZW5ndGgpIHtcbiAgICAgICAgY29weS5fYXR0cmlidXRlcyA9IHRoaXMuYXR0cmlidXRlcy5jbG9uZSgpO1xuICAgICAgfVxuXG4gICAgICBpZiAodGhpcy5jb250ZW50KSB7XG4gICAgICAgIGlmICh0aGlzLmNvbnRlbnQuY2xvbmUpIHtcbiAgICAgICAgICBjb3B5LmNvbnRlbnQgPSB0aGlzLmNvbnRlbnQuY2xvbmUoKTtcbiAgICAgICAgfSBlbHNlIGlmIChBcnJheS5pc0FycmF5KHRoaXMuY29udGVudCkpIHtcbiAgICAgICAgICBjb3B5LmNvbnRlbnQgPSB0aGlzLmNvbnRlbnQubWFwKGZ1bmN0aW9uIChlbGVtZW50KSB7XG4gICAgICAgICAgICByZXR1cm4gZWxlbWVudC5jbG9uZSgpO1xuICAgICAgICAgIH0pO1xuICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgIGNvcHkuY29udGVudCA9IHRoaXMuY29udGVudDtcbiAgICAgICAgfVxuICAgICAgfSBlbHNlIHtcbiAgICAgICAgY29weS5jb250ZW50ID0gdGhpcy5jb250ZW50O1xuICAgICAgfVxuXG4gICAgICByZXR1cm4gY29weTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAndG9WYWx1ZScsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIHRvVmFsdWUoKSB7XG4gICAgICBpZiAodGhpcy5jb250ZW50IGluc3RhbmNlb2YgRWxlbWVudCkge1xuICAgICAgICByZXR1cm4gdGhpcy5jb250ZW50LnRvVmFsdWUoKTtcbiAgICAgIH1cblxuICAgICAgaWYgKHRoaXMuY29udGVudCBpbnN0YW5jZW9mIEtleVZhbHVlUGFpcikge1xuICAgICAgICByZXR1cm4ge1xuICAgICAgICAgIGtleTogdGhpcy5jb250ZW50LmtleS50b1ZhbHVlKCksXG4gICAgICAgICAgdmFsdWU6IHRoaXMuY29udGVudC52YWx1ZSA/IHRoaXMuY29udGVudC52YWx1ZS50b1ZhbHVlKCkgOiB1bmRlZmluZWRcbiAgICAgICAgfTtcbiAgICAgIH1cblxuICAgICAgaWYgKHRoaXMuY29udGVudCAmJiB0aGlzLmNvbnRlbnQubWFwKSB7XG4gICAgICAgIHJldHVybiB0aGlzLmNvbnRlbnQubWFwKGZ1bmN0aW9uIChlbGVtZW50KSB7XG4gICAgICAgICAgcmV0dXJuIGVsZW1lbnQudG9WYWx1ZSgpO1xuICAgICAgICB9LCB0aGlzKTtcbiAgICAgIH1cblxuICAgICAgcmV0dXJuIHRoaXMuY29udGVudDtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBDcmVhdGVzIGEgcmVmZXJlbmNlIHBvaW50aW5nIGF0IHRoZSBFbGVtZW50XG4gICAgICogQHJldHVybnMge1JlZkVsZW1lbnR9XG4gICAgICogQG1lbWJlcm9mIEVsZW1lbnQucHJvdG90eXBlXG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ3RvUmVmJyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gdG9SZWYocGF0aCkge1xuICAgICAgaWYgKHRoaXMuaWQudG9WYWx1ZSgpID09PSAnJykge1xuICAgICAgICB0aHJvdyBFcnJvcignQ2Fubm90IGNyZWF0ZSByZWZlcmVuY2UgdG8gYW4gZWxlbWVudCB0aGF0IGRvZXMgbm90IGNvbnRhaW4gYW4gSUQnKTtcbiAgICAgIH1cblxuICAgICAgdmFyIHJlZiA9IG5ldyB0aGlzLlJlZkVsZW1lbnQodGhpcy5pZC50b1ZhbHVlKCkpO1xuXG4gICAgICBpZiAocGF0aCkge1xuICAgICAgICByZWYucGF0aCA9IHBhdGg7XG4gICAgICB9XG5cbiAgICAgIHJldHVybiByZWY7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogRmluZHMgdGhlIGdpdmVuIGVsZW1lbnRzIGluIHRoZSBlbGVtZW50IHRyZWUuXG4gICAgICogV2hlbiBwcm92aWRpbmcgbXVsdGlwbGUgZWxlbWVudCBuYW1lcywgeW91IG11c3QgZmlyc3QgZnJlZXplIHRoZSBlbGVtZW50LlxuICAgICAqXG4gICAgICogQHBhcmFtIG5hbWVzIHsuLi5lbGVtZW50TmFtZXN9XG4gICAgICogQHJldHVybnMge0FycmF5U2xpY2V9XG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ2ZpbmRSZWN1cnNpdmUnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBmaW5kUmVjdXJzaXZlKCkge1xuICAgICAgZm9yICh2YXIgX2xlbiA9IGFyZ3VtZW50cy5sZW5ndGgsIGVsZW1lbnROYW1lcyA9IEFycmF5KF9sZW4pLCBfa2V5ID0gMDsgX2tleSA8IF9sZW47IF9rZXkrKykge1xuICAgICAgICBlbGVtZW50TmFtZXNbX2tleV0gPSBhcmd1bWVudHNbX2tleV07XG4gICAgICB9XG5cbiAgICAgIGlmIChhcmd1bWVudHMubGVuZ3RoID4gMSAmJiAhdGhpcy5pc0Zyb3plbikge1xuICAgICAgICB0aHJvdyBuZXcgRXJyb3IoJ0Nhbm5vdCBmaW5kIHJlY3Vyc2l2ZSB3aXRoIG11bHRpcGxlIGVsZW1lbnQgbmFtZXMgd2l0aG91dCBmaXJzdCBmcmVlemluZyB0aGUgZWxlbWVudC4gQ2FsbCBgZWxlbWVudC5mcmVlemUoKWAnKTtcbiAgICAgIH1cblxuICAgICAgdmFyIGVsZW1lbnROYW1lID0gZWxlbWVudE5hbWVzLnBvcCgpO1xuICAgICAgdmFyIGVsZW1lbnRzID0gbmV3IEFycmF5U2xpY2UoKTtcblxuICAgICAgdmFyIGFwcGVuZCA9IGZ1bmN0aW9uIGFwcGVuZChhcnJheSwgZWxlbWVudCkge1xuICAgICAgICBhcnJheS5wdXNoKGVsZW1lbnQpO1xuICAgICAgICByZXR1cm4gYXJyYXk7XG4gICAgICB9O1xuXG4gICAgICAvLyBDaGVja3MgdGhlIGdpdmVuIGVsZW1lbnQgYW5kIGFwcGVuZHMgZWxlbWVudC9zdWItZWxlbWVudHNcbiAgICAgIC8vIHRoYXQgbWF0Y2ggZWxlbWVudCBuYW1lIHRvIGdpdmVuIGFycmF5XG4gICAgICB2YXIgY2hlY2tFbGVtZW50ID0gZnVuY3Rpb24gY2hlY2tFbGVtZW50KGFycmF5LCBlbGVtZW50KSB7XG4gICAgICAgIGlmIChlbGVtZW50LmVsZW1lbnQgPT09IGVsZW1lbnROYW1lKSB7XG4gICAgICAgICAgYXJyYXkucHVzaChlbGVtZW50KTtcbiAgICAgICAgfVxuXG4gICAgICAgIHZhciBpdGVtcyA9IGVsZW1lbnQuZmluZFJlY3Vyc2l2ZShlbGVtZW50TmFtZSk7XG4gICAgICAgIGlmIChpdGVtcykge1xuICAgICAgICAgIGl0ZW1zLnJlZHVjZShhcHBlbmQsIGFycmF5KTtcbiAgICAgICAgfVxuXG4gICAgICAgIGlmIChlbGVtZW50LmNvbnRlbnQgaW5zdGFuY2VvZiBLZXlWYWx1ZVBhaXIpIHtcbiAgICAgICAgICBpZiAoZWxlbWVudC5jb250ZW50LmtleSkge1xuICAgICAgICAgICAgY2hlY2tFbGVtZW50KGFycmF5LCBlbGVtZW50LmNvbnRlbnQua2V5KTtcbiAgICAgICAgICB9XG5cbiAgICAgICAgICBpZiAoZWxlbWVudC5jb250ZW50LnZhbHVlKSB7XG4gICAgICAgICAgICBjaGVja0VsZW1lbnQoYXJyYXksIGVsZW1lbnQuY29udGVudC52YWx1ZSk7XG4gICAgICAgICAgfVxuICAgICAgICB9XG5cbiAgICAgICAgcmV0dXJuIGFycmF5O1xuICAgICAgfTtcblxuICAgICAgaWYgKHRoaXMuY29udGVudCkge1xuICAgICAgICAvLyBEaXJlY3QgRWxlbWVudFxuICAgICAgICBpZiAodGhpcy5jb250ZW50LmVsZW1lbnQpIHtcbiAgICAgICAgICBjaGVja0VsZW1lbnQoZWxlbWVudHMsIHRoaXMuY29udGVudCk7XG4gICAgICAgIH1cblxuICAgICAgICAvLyBFbGVtZW50IEFycmF5XG4gICAgICAgIGlmIChBcnJheS5pc0FycmF5KHRoaXMuY29udGVudCkpIHtcbiAgICAgICAgICB0aGlzLmNvbnRlbnQucmVkdWNlKGNoZWNrRWxlbWVudCwgZWxlbWVudHMpO1xuICAgICAgICB9XG4gICAgICB9XG5cbiAgICAgIGlmICghZWxlbWVudE5hbWVzLmlzRW1wdHkpIHtcbiAgICAgICAgZWxlbWVudHMgPSBlbGVtZW50cy5maWx0ZXIoZnVuY3Rpb24gKGVsZW1lbnQpIHtcbiAgICAgICAgICB2YXIgcGFyZW50RWxlbWVudHMgPSBlbGVtZW50LnBhcmVudHMubWFwKGZ1bmN0aW9uIChlKSB7XG4gICAgICAgICAgICByZXR1cm4gZS5lbGVtZW50O1xuICAgICAgICAgIH0pO1xuXG4gICAgICAgICAgLy8gZXNsaW50LWRpc2FibGUtbmV4dC1saW5lIG5vLXJlc3RyaWN0ZWQtc3ludGF4XG4gICAgICAgICAgZm9yICh2YXIgbmFtZXNJbmRleCBpbiBlbGVtZW50TmFtZXMpIHtcbiAgICAgICAgICAgIHZhciBuYW1lID0gZWxlbWVudE5hbWVzW25hbWVzSW5kZXhdO1xuICAgICAgICAgICAgdmFyIGluZGV4ID0gcGFyZW50RWxlbWVudHMuaW5kZXhPZihuYW1lKTtcblxuICAgICAgICAgICAgaWYgKGluZGV4ICE9PSAtMSkge1xuICAgICAgICAgICAgICBwYXJlbnRFbGVtZW50cyA9IHBhcmVudEVsZW1lbnRzLnNwbGljZSgwLCBpbmRleCk7XG4gICAgICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgICAgICByZXR1cm4gZmFsc2U7XG4gICAgICAgICAgICB9XG4gICAgICAgICAgfVxuXG4gICAgICAgICAgcmV0dXJuIHRydWU7XG4gICAgICAgIH0pO1xuICAgICAgfVxuXG4gICAgICByZXR1cm4gZWxlbWVudHM7XG4gICAgfVxuICB9LCB7XG4gICAga2V5OiAnc2V0JyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gc2V0KGNvbnRlbnQpIHtcbiAgICAgIHRoaXMuY29udGVudCA9IGNvbnRlbnQ7XG4gICAgICByZXR1cm4gdGhpcztcbiAgICB9XG4gIH0sIHtcbiAgICBrZXk6ICdlcXVhbHMnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBlcXVhbHModmFsdWUpIHtcbiAgICAgIHJldHVybiBpc0VxdWFsKHRoaXMudG9WYWx1ZSgpLCB2YWx1ZSk7XG4gICAgfVxuICB9LCB7XG4gICAga2V5OiAnZ2V0TWV0YVByb3BlcnR5JyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gZ2V0TWV0YVByb3BlcnR5KG5hbWUsIHZhbHVlKSB7XG4gICAgICBpZiAoIXRoaXMubWV0YS5oYXNLZXkobmFtZSkpIHtcbiAgICAgICAgaWYgKHRoaXMuaXNGcm96ZW4pIHtcbiAgICAgICAgICB2YXIgZWxlbWVudCA9IHRoaXMucmVmcmFjdCh2YWx1ZSk7XG4gICAgICAgICAgZWxlbWVudC5mcmVlemUoKTtcbiAgICAgICAgICByZXR1cm4gZWxlbWVudDtcbiAgICAgICAgfVxuXG4gICAgICAgIHRoaXMubWV0YS5zZXQobmFtZSwgdmFsdWUpO1xuICAgICAgfVxuXG4gICAgICByZXR1cm4gdGhpcy5tZXRhLmdldChuYW1lKTtcbiAgICB9XG4gIH0sIHtcbiAgICBrZXk6ICdzZXRNZXRhUHJvcGVydHknLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBzZXRNZXRhUHJvcGVydHkobmFtZSwgdmFsdWUpIHtcbiAgICAgIHRoaXMubWV0YS5zZXQobmFtZSwgdmFsdWUpO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEB0eXBlIFN0cmluZ1xuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdlbGVtZW50JyxcbiAgICBnZXQ6IGZ1bmN0aW9uIGdldCgpIHtcbiAgICAgIC8vIFJldHVybnMgJ2VsZW1lbnQnIHNvIHdlIGRvbid0IGhhdmUgdW5kZWZpbmVkIGFzIGVsZW1lbnRcbiAgICAgIHJldHVybiB0aGlzLl9zdG9yZWRFbGVtZW50IHx8ICdlbGVtZW50JztcbiAgICB9LFxuICAgIHNldDogZnVuY3Rpb24gc2V0KGVsZW1lbnQpIHtcbiAgICAgIHRoaXMuX3N0b3JlZEVsZW1lbnQgPSBlbGVtZW50O1xuICAgIH1cbiAgfSwge1xuICAgIGtleTogJ2NvbnRlbnQnLFxuICAgIGdldDogZnVuY3Rpb24gZ2V0KCkge1xuICAgICAgcmV0dXJuIHRoaXMuX2NvbnRlbnQ7XG4gICAgfSxcbiAgICBzZXQ6IGZ1bmN0aW9uIHNldCh2YWx1ZSkge1xuICAgICAgdmFyIF90aGlzMiA9IHRoaXM7XG5cbiAgICAgIGlmICh2YWx1ZSBpbnN0YW5jZW9mIEVsZW1lbnQpIHtcbiAgICAgICAgdGhpcy5fY29udGVudCA9IHZhbHVlO1xuICAgICAgfSBlbHNlIGlmICh2YWx1ZSBpbnN0YW5jZW9mIEFycmF5U2xpY2UpIHtcbiAgICAgICAgdGhpcy5jb250ZW50ID0gdmFsdWUuZWxlbWVudHM7XG4gICAgICB9IGVsc2UgaWYgKHR5cGVvZiB2YWx1ZSA9PSAnc3RyaW5nJyB8fCB0eXBlb2YgdmFsdWUgPT0gJ251bWJlcicgfHwgdHlwZW9mIHZhbHVlID09ICdib29sZWFuJyB8fCB2YWx1ZSA9PT0gJ251bGwnIHx8IHZhbHVlID09IHVuZGVmaW5lZCkge1xuICAgICAgICAvLyBQcmltaXRpdmUgVmFsdWVzXG4gICAgICAgIHRoaXMuX2NvbnRlbnQgPSB2YWx1ZTtcbiAgICAgIH0gZWxzZSBpZiAodmFsdWUgaW5zdGFuY2VvZiBLZXlWYWx1ZVBhaXIpIHtcbiAgICAgICAgdGhpcy5fY29udGVudCA9IHZhbHVlO1xuICAgICAgfSBlbHNlIGlmIChBcnJheS5pc0FycmF5KHZhbHVlKSkge1xuICAgICAgICB0aGlzLl9jb250ZW50ID0gdmFsdWUubWFwKHRoaXMucmVmcmFjdCk7XG4gICAgICB9IGVsc2UgaWYgKCh0eXBlb2YgdmFsdWUgPT09ICd1bmRlZmluZWQnID8gJ3VuZGVmaW5lZCcgOiBfdHlwZW9mKHZhbHVlKSkgPT09ICdvYmplY3QnKSB7XG4gICAgICAgIHRoaXMuX2NvbnRlbnQgPSBPYmplY3Qua2V5cyh2YWx1ZSkubWFwKGZ1bmN0aW9uIChrZXkpIHtcbiAgICAgICAgICByZXR1cm4gbmV3IF90aGlzMi5NZW1iZXJFbGVtZW50KGtleSwgdmFsdWVba2V5XSk7XG4gICAgICAgIH0pO1xuICAgICAgfSBlbHNlIHtcbiAgICAgICAgdGhyb3cgbmV3IEVycm9yKCdDYW5ub3Qgc2V0IGNvbnRlbnQgdG8gZ2l2ZW4gdmFsdWUnKTtcbiAgICAgIH1cbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBAdHlwZSBPYmplY3RFbGVtZW50XG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ21ldGEnLFxuICAgIGdldDogZnVuY3Rpb24gZ2V0KCkge1xuICAgICAgaWYgKCF0aGlzLl9tZXRhKSB7XG4gICAgICAgIGlmICh0aGlzLmlzRnJvemVuKSB7XG4gICAgICAgICAgdmFyIG1ldGEgPSBuZXcgdGhpcy5PYmplY3RFbGVtZW50KCk7XG4gICAgICAgICAgbWV0YS5mcmVlemUoKTtcbiAgICAgICAgICByZXR1cm4gbWV0YTtcbiAgICAgICAgfVxuXG4gICAgICAgIHRoaXMuX21ldGEgPSBuZXcgdGhpcy5PYmplY3RFbGVtZW50KCk7XG4gICAgICB9XG5cbiAgICAgIHJldHVybiB0aGlzLl9tZXRhO1xuICAgIH0sXG4gICAgc2V0OiBmdW5jdGlvbiBzZXQodmFsdWUpIHtcbiAgICAgIGlmICh2YWx1ZSBpbnN0YW5jZW9mIHRoaXMuT2JqZWN0RWxlbWVudCkge1xuICAgICAgICB0aGlzLl9tZXRhID0gdmFsdWU7XG4gICAgICB9IGVsc2Uge1xuICAgICAgICB0aGlzLm1ldGEuc2V0KHZhbHVlIHx8IHt9KTtcbiAgICAgIH1cbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBUaGUgYXR0cmlidXRlcyBwcm9wZXJ0eSBkZWZpbmVzIGF0dHJpYnV0ZXMgYWJvdXQgdGhlIGdpdmVuIGluc3RhbmNlXG4gICAgICogb2YgdGhlIGVsZW1lbnQsIGFzIHNwZWNpZmllZCBieSB0aGUgZWxlbWVudCBwcm9wZXJ0eS5cbiAgICAgKlxuICAgICAqIEB0eXBlIE9iamVjdEVsZW1lbnRcbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAnYXR0cmlidXRlcycsXG4gICAgZ2V0OiBmdW5jdGlvbiBnZXQoKSB7XG4gICAgICBpZiAoIXRoaXMuX2F0dHJpYnV0ZXMpIHtcbiAgICAgICAgaWYgKHRoaXMuaXNGcm96ZW4pIHtcbiAgICAgICAgICB2YXIgbWV0YSA9IG5ldyB0aGlzLk9iamVjdEVsZW1lbnQoKTtcbiAgICAgICAgICBtZXRhLmZyZWV6ZSgpO1xuICAgICAgICAgIHJldHVybiBtZXRhO1xuICAgICAgICB9XG5cbiAgICAgICAgdGhpcy5fYXR0cmlidXRlcyA9IG5ldyB0aGlzLk9iamVjdEVsZW1lbnQoKTtcbiAgICAgIH1cblxuICAgICAgcmV0dXJuIHRoaXMuX2F0dHJpYnV0ZXM7XG4gICAgfSxcbiAgICBzZXQ6IGZ1bmN0aW9uIHNldCh2YWx1ZSkge1xuICAgICAgaWYgKHZhbHVlIGluc3RhbmNlb2YgdGhpcy5PYmplY3RFbGVtZW50KSB7XG4gICAgICAgIHRoaXMuX2F0dHJpYnV0ZXMgPSB2YWx1ZTtcbiAgICAgIH0gZWxzZSB7XG4gICAgICAgIHRoaXMuYXR0cmlidXRlcy5zZXQodmFsdWUgfHwge30pO1xuICAgICAgfVxuICAgIH1cblxuICAgIC8qKlxuICAgICAqIFVuaXF1ZSBJZGVudGlmaWVyLCBNVVNUIGJlIHVuaXF1ZSB0aHJvdWdob3V0IGFuIGVudGlyZSBlbGVtZW50IHRyZWUuXG4gICAgICogQHR5cGUgU3RyaW5nRWxlbWVudFxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdpZCcsXG4gICAgZ2V0OiBmdW5jdGlvbiBnZXQoKSB7XG4gICAgICByZXR1cm4gdGhpcy5nZXRNZXRhUHJvcGVydHkoJ2lkJywgJycpO1xuICAgIH0sXG4gICAgc2V0OiBmdW5jdGlvbiBzZXQoZWxlbWVudCkge1xuICAgICAgdGhpcy5zZXRNZXRhUHJvcGVydHkoJ2lkJywgZWxlbWVudCk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQHR5cGUgQXJyYXlFbGVtZW50XG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ2NsYXNzZXMnLFxuICAgIGdldDogZnVuY3Rpb24gZ2V0KCkge1xuICAgICAgcmV0dXJuIHRoaXMuZ2V0TWV0YVByb3BlcnR5KCdjbGFzc2VzJywgW10pO1xuICAgIH0sXG4gICAgc2V0OiBmdW5jdGlvbiBzZXQoZWxlbWVudCkge1xuICAgICAgdGhpcy5zZXRNZXRhUHJvcGVydHkoJ2NsYXNzZXMnLCBlbGVtZW50KTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBIdW1hbi1yZWFkYWJsZSB0aXRsZSBvZiBlbGVtZW50XG4gICAgICogQHR5cGUgU3RyaW5nRWxlbWVudFxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICd0aXRsZScsXG4gICAgZ2V0OiBmdW5jdGlvbiBnZXQoKSB7XG4gICAgICByZXR1cm4gdGhpcy5nZXRNZXRhUHJvcGVydHkoJ3RpdGxlJywgJycpO1xuICAgIH0sXG4gICAgc2V0OiBmdW5jdGlvbiBzZXQoZWxlbWVudCkge1xuICAgICAgdGhpcy5zZXRNZXRhUHJvcGVydHkoJ3RpdGxlJywgZWxlbWVudCk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogSHVtYW4tcmVhZGFibGUgZGVzY3JpcHRpb24gb2YgZWxlbWVudFxuICAgICAqIEB0eXBlIFN0cmluZ0VsZW1lbnRcbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAnZGVzY3JpcHRpb24nLFxuICAgIGdldDogZnVuY3Rpb24gZ2V0KCkge1xuICAgICAgcmV0dXJuIHRoaXMuZ2V0TWV0YVByb3BlcnR5KCdkZXNjcmlwdGlvbicsICcnKTtcbiAgICB9LFxuICAgIHNldDogZnVuY3Rpb24gc2V0KGVsZW1lbnQpIHtcbiAgICAgIHRoaXMuc2V0TWV0YVByb3BlcnR5KCdkZXNjcmlwdGlvbicsIGVsZW1lbnQpO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEB0eXBlIEFycmF5RWxlbWVudFxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdsaW5rcycsXG4gICAgZ2V0OiBmdW5jdGlvbiBnZXQoKSB7XG4gICAgICByZXR1cm4gdGhpcy5nZXRNZXRhUHJvcGVydHkoJ2xpbmtzJywgW10pO1xuICAgIH0sXG4gICAgc2V0OiBmdW5jdGlvbiBzZXQoZWxlbWVudCkge1xuICAgICAgdGhpcy5zZXRNZXRhUHJvcGVydHkoJ2xpbmtzJywgZWxlbWVudCk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogUmV0dXJucyB3aGV0aGVyIHRoZSBlbGVtZW50IGlzIGZyb3plbi5cbiAgICAgKiBAdHlwZSBib29sZWFuXG4gICAgICogQHNlZSBmcmVlemVcbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAnaXNGcm96ZW4nLFxuICAgIGdldDogZnVuY3Rpb24gZ2V0KCkge1xuICAgICAgcmV0dXJuIE9iamVjdC5pc0Zyb3plbih0aGlzKTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBSZXR1cm5zIGFsbCBvZiB0aGUgcGFyZW50IGVsZW1lbnRzLlxuICAgICAqIEB0eXBlIEFycmF5U2xpY2VcbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAncGFyZW50cycsXG4gICAgZ2V0OiBmdW5jdGlvbiBnZXQoKSB7XG4gICAgICB2YXIgcGFyZW50ID0gdGhpcy5wYXJlbnQ7XG5cbiAgICAgIHZhciBwYXJlbnRzID0gbmV3IEFycmF5U2xpY2UoKTtcblxuICAgICAgd2hpbGUgKHBhcmVudCkge1xuICAgICAgICBwYXJlbnRzLnB1c2gocGFyZW50KTtcblxuICAgICAgICAvLyBlc2xpbnQtZGlzYWJsZS1uZXh0LWxpbmUgcHJlZmVyLWRlc3RydWN0dXJpbmdcbiAgICAgICAgcGFyZW50ID0gcGFyZW50LnBhcmVudDtcbiAgICAgIH1cblxuICAgICAgcmV0dXJuIHBhcmVudHM7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogUmV0dXJucyBhbGwgb2YgdGhlIGNoaWxkcmVuIGVsZW1lbnRzIGZvdW5kIHdpdGhpbiB0aGUgZWxlbWVudC5cbiAgICAgKiBAdHlwZSBBcnJheVNsaWNlXG4gICAgICogQHNlZSByZWN1cnNpdmVDaGlsZHJlblxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdjaGlsZHJlbicsXG4gICAgZ2V0OiBmdW5jdGlvbiBnZXQoKSB7XG4gICAgICBpZiAoQXJyYXkuaXNBcnJheSh0aGlzLmNvbnRlbnQpKSB7XG4gICAgICAgIHJldHVybiBuZXcgQXJyYXlTbGljZSh0aGlzLmNvbnRlbnQpO1xuICAgICAgfVxuXG4gICAgICBpZiAodGhpcy5jb250ZW50IGluc3RhbmNlb2YgS2V5VmFsdWVQYWlyKSB7XG4gICAgICAgIHZhciBjaGlsZHJlbiA9IG5ldyBBcnJheVNsaWNlKFt0aGlzLmNvbnRlbnQua2V5XSk7XG5cbiAgICAgICAgaWYgKHRoaXMuY29udGVudC52YWx1ZSkge1xuICAgICAgICAgIGNoaWxkcmVuLnB1c2godGhpcy5jb250ZW50LnZhbHVlKTtcbiAgICAgICAgfVxuXG4gICAgICAgIHJldHVybiBjaGlsZHJlbjtcbiAgICAgIH1cblxuICAgICAgaWYgKHRoaXMuY29udGVudCBpbnN0YW5jZW9mIEVsZW1lbnQpIHtcbiAgICAgICAgcmV0dXJuIG5ldyBBcnJheVNsaWNlKFt0aGlzLmNvbnRlbnRdKTtcbiAgICAgIH1cblxuICAgICAgcmV0dXJuIG5ldyBBcnJheVNsaWNlKCk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgKiBSZXR1cm5zIGFsbCBvZiB0aGUgY2hpbGRyZW4gZWxlbWVudHMgZm91bmQgd2l0aGluIHRoZSBlbGVtZW50IHJlY3Vyc2l2ZWx5LlxuICAgICogQHR5cGUgQXJyYXlTbGljZVxuICAgICogQHNlZSBjaGlsZHJlblxuICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ3JlY3Vyc2l2ZUNoaWxkcmVuJyxcbiAgICBnZXQ6IGZ1bmN0aW9uIGdldCgpIHtcbiAgICAgIHZhciBjaGlsZHJlbiA9IG5ldyBBcnJheVNsaWNlKCk7XG5cbiAgICAgIHRoaXMuY2hpbGRyZW4uZm9yRWFjaChmdW5jdGlvbiAoZWxlbWVudCkge1xuICAgICAgICBjaGlsZHJlbi5wdXNoKGVsZW1lbnQpO1xuXG4gICAgICAgIGVsZW1lbnQucmVjdXJzaXZlQ2hpbGRyZW4uZm9yRWFjaChmdW5jdGlvbiAoY2hpbGQpIHtcbiAgICAgICAgICBjaGlsZHJlbi5wdXNoKGNoaWxkKTtcbiAgICAgICAgfSk7XG4gICAgICB9KTtcblxuICAgICAgcmV0dXJuIGNoaWxkcmVuO1xuICAgIH1cbiAgfV0pO1xuXG4gIHJldHVybiBFbGVtZW50O1xufSgpO1xuXG5tb2R1bGUuZXhwb3J0cyA9IEVsZW1lbnQ7IiwiJ3VzZSBzdHJpY3QnO1xuXG52YXIgX2NyZWF0ZUNsYXNzID0gZnVuY3Rpb24gKCkgeyBmdW5jdGlvbiBkZWZpbmVQcm9wZXJ0aWVzKHRhcmdldCwgcHJvcHMpIHsgZm9yICh2YXIgaSA9IDA7IGkgPCBwcm9wcy5sZW5ndGg7IGkrKykgeyB2YXIgZGVzY3JpcHRvciA9IHByb3BzW2ldOyBkZXNjcmlwdG9yLmVudW1lcmFibGUgPSBkZXNjcmlwdG9yLmVudW1lcmFibGUgfHwgZmFsc2U7IGRlc2NyaXB0b3IuY29uZmlndXJhYmxlID0gdHJ1ZTsgaWYgKFwidmFsdWVcIiBpbiBkZXNjcmlwdG9yKSBkZXNjcmlwdG9yLndyaXRhYmxlID0gdHJ1ZTsgT2JqZWN0LmRlZmluZVByb3BlcnR5KHRhcmdldCwgZGVzY3JpcHRvci5rZXksIGRlc2NyaXB0b3IpOyB9IH0gcmV0dXJuIGZ1bmN0aW9uIChDb25zdHJ1Y3RvciwgcHJvdG9Qcm9wcywgc3RhdGljUHJvcHMpIHsgaWYgKHByb3RvUHJvcHMpIGRlZmluZVByb3BlcnRpZXMoQ29uc3RydWN0b3IucHJvdG90eXBlLCBwcm90b1Byb3BzKTsgaWYgKHN0YXRpY1Byb3BzKSBkZWZpbmVQcm9wZXJ0aWVzKENvbnN0cnVjdG9yLCBzdGF0aWNQcm9wcyk7IHJldHVybiBDb25zdHJ1Y3RvcjsgfTsgfSgpO1xuXG5mdW5jdGlvbiBfY2xhc3NDYWxsQ2hlY2soaW5zdGFuY2UsIENvbnN0cnVjdG9yKSB7IGlmICghKGluc3RhbmNlIGluc3RhbmNlb2YgQ29uc3RydWN0b3IpKSB7IHRocm93IG5ldyBUeXBlRXJyb3IoXCJDYW5ub3QgY2FsbCBhIGNsYXNzIGFzIGEgZnVuY3Rpb25cIik7IH0gfVxuXG5mdW5jdGlvbiBfcG9zc2libGVDb25zdHJ1Y3RvclJldHVybihzZWxmLCBjYWxsKSB7IGlmICghc2VsZikgeyB0aHJvdyBuZXcgUmVmZXJlbmNlRXJyb3IoXCJ0aGlzIGhhc24ndCBiZWVuIGluaXRpYWxpc2VkIC0gc3VwZXIoKSBoYXNuJ3QgYmVlbiBjYWxsZWRcIik7IH0gcmV0dXJuIGNhbGwgJiYgKHR5cGVvZiBjYWxsID09PSBcIm9iamVjdFwiIHx8IHR5cGVvZiBjYWxsID09PSBcImZ1bmN0aW9uXCIpID8gY2FsbCA6IHNlbGY7IH1cblxuZnVuY3Rpb24gX2luaGVyaXRzKHN1YkNsYXNzLCBzdXBlckNsYXNzKSB7IGlmICh0eXBlb2Ygc3VwZXJDbGFzcyAhPT0gXCJmdW5jdGlvblwiICYmIHN1cGVyQ2xhc3MgIT09IG51bGwpIHsgdGhyb3cgbmV3IFR5cGVFcnJvcihcIlN1cGVyIGV4cHJlc3Npb24gbXVzdCBlaXRoZXIgYmUgbnVsbCBvciBhIGZ1bmN0aW9uLCBub3QgXCIgKyB0eXBlb2Ygc3VwZXJDbGFzcyk7IH0gc3ViQ2xhc3MucHJvdG90eXBlID0gT2JqZWN0LmNyZWF0ZShzdXBlckNsYXNzICYmIHN1cGVyQ2xhc3MucHJvdG90eXBlLCB7IGNvbnN0cnVjdG9yOiB7IHZhbHVlOiBzdWJDbGFzcywgZW51bWVyYWJsZTogZmFsc2UsIHdyaXRhYmxlOiB0cnVlLCBjb25maWd1cmFibGU6IHRydWUgfSB9KTsgaWYgKHN1cGVyQ2xhc3MpIE9iamVjdC5zZXRQcm90b3R5cGVPZiA/IE9iamVjdC5zZXRQcm90b3R5cGVPZihzdWJDbGFzcywgc3VwZXJDbGFzcykgOiBzdWJDbGFzcy5fX3Byb3RvX18gPSBzdXBlckNsYXNzOyB9XG5cbnZhciBLZXlWYWx1ZVBhaXIgPSByZXF1aXJlKCcuLi9LZXlWYWx1ZVBhaXInKTtcbnZhciBFbGVtZW50ID0gcmVxdWlyZSgnLi9FbGVtZW50Jyk7XG5cbi8qKlxuICogQGNsYXNzIE1lbWJlckVsZW1lbnRcbiAqXG4gKiBAcGFyYW0ge0VsZW1lbnR9IGtleVxuICogQHBhcmFtIHtFbGVtZW50fSB2YWx1ZVxuICogQHBhcmFtIG1ldGFcbiAqIEBwYXJhbSBhdHRyaWJ1dGVzXG4gKi9cbm1vZHVsZS5leHBvcnRzID0gZnVuY3Rpb24gKF9FbGVtZW50KSB7XG4gIF9pbmhlcml0cyhNZW1iZXJFbGVtZW50LCBfRWxlbWVudCk7XG5cbiAgZnVuY3Rpb24gTWVtYmVyRWxlbWVudChrZXksIHZhbHVlLCBtZXRhLCBhdHRyaWJ1dGVzKSB7XG4gICAgX2NsYXNzQ2FsbENoZWNrKHRoaXMsIE1lbWJlckVsZW1lbnQpO1xuXG4gICAgdmFyIF90aGlzID0gX3Bvc3NpYmxlQ29uc3RydWN0b3JSZXR1cm4odGhpcywgKE1lbWJlckVsZW1lbnQuX19wcm90b19fIHx8IE9iamVjdC5nZXRQcm90b3R5cGVPZihNZW1iZXJFbGVtZW50KSkuY2FsbCh0aGlzLCBuZXcgS2V5VmFsdWVQYWlyKCksIG1ldGEsIGF0dHJpYnV0ZXMpKTtcblxuICAgIF90aGlzLmVsZW1lbnQgPSAnbWVtYmVyJztcbiAgICBfdGhpcy5rZXkgPSBrZXk7XG4gICAgX3RoaXMudmFsdWUgPSB2YWx1ZTtcbiAgICByZXR1cm4gX3RoaXM7XG4gIH1cblxuICAvKipcbiAgICogQHR5cGUgRWxlbWVudFxuICAgKi9cblxuXG4gIF9jcmVhdGVDbGFzcyhNZW1iZXJFbGVtZW50LCBbe1xuICAgIGtleTogJ2tleScsXG4gICAgZ2V0OiBmdW5jdGlvbiBnZXQoKSB7XG4gICAgICByZXR1cm4gdGhpcy5jb250ZW50LmtleTtcbiAgICB9LFxuICAgIHNldDogZnVuY3Rpb24gc2V0KGtleSkge1xuICAgICAgdGhpcy5jb250ZW50LmtleSA9IHRoaXMucmVmcmFjdChrZXkpO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEB0eXBlIEVsZW1lbnRcbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAndmFsdWUnLFxuICAgIGdldDogZnVuY3Rpb24gZ2V0KCkge1xuICAgICAgcmV0dXJuIHRoaXMuY29udGVudC52YWx1ZTtcbiAgICB9LFxuICAgIHNldDogZnVuY3Rpb24gc2V0KHZhbHVlKSB7XG4gICAgICB0aGlzLmNvbnRlbnQudmFsdWUgPSB0aGlzLnJlZnJhY3QodmFsdWUpO1xuICAgIH1cbiAgfV0pO1xuXG4gIHJldHVybiBNZW1iZXJFbGVtZW50O1xufShFbGVtZW50KTsiLCIndXNlIHN0cmljdCc7XG5cbnZhciBfY3JlYXRlQ2xhc3MgPSBmdW5jdGlvbiAoKSB7IGZ1bmN0aW9uIGRlZmluZVByb3BlcnRpZXModGFyZ2V0LCBwcm9wcykgeyBmb3IgKHZhciBpID0gMDsgaSA8IHByb3BzLmxlbmd0aDsgaSsrKSB7IHZhciBkZXNjcmlwdG9yID0gcHJvcHNbaV07IGRlc2NyaXB0b3IuZW51bWVyYWJsZSA9IGRlc2NyaXB0b3IuZW51bWVyYWJsZSB8fCBmYWxzZTsgZGVzY3JpcHRvci5jb25maWd1cmFibGUgPSB0cnVlOyBpZiAoXCJ2YWx1ZVwiIGluIGRlc2NyaXB0b3IpIGRlc2NyaXB0b3Iud3JpdGFibGUgPSB0cnVlOyBPYmplY3QuZGVmaW5lUHJvcGVydHkodGFyZ2V0LCBkZXNjcmlwdG9yLmtleSwgZGVzY3JpcHRvcik7IH0gfSByZXR1cm4gZnVuY3Rpb24gKENvbnN0cnVjdG9yLCBwcm90b1Byb3BzLCBzdGF0aWNQcm9wcykgeyBpZiAocHJvdG9Qcm9wcykgZGVmaW5lUHJvcGVydGllcyhDb25zdHJ1Y3Rvci5wcm90b3R5cGUsIHByb3RvUHJvcHMpOyBpZiAoc3RhdGljUHJvcHMpIGRlZmluZVByb3BlcnRpZXMoQ29uc3RydWN0b3IsIHN0YXRpY1Byb3BzKTsgcmV0dXJuIENvbnN0cnVjdG9yOyB9OyB9KCk7XG5cbmZ1bmN0aW9uIF9jbGFzc0NhbGxDaGVjayhpbnN0YW5jZSwgQ29uc3RydWN0b3IpIHsgaWYgKCEoaW5zdGFuY2UgaW5zdGFuY2VvZiBDb25zdHJ1Y3RvcikpIHsgdGhyb3cgbmV3IFR5cGVFcnJvcihcIkNhbm5vdCBjYWxsIGEgY2xhc3MgYXMgYSBmdW5jdGlvblwiKTsgfSB9XG5cbmZ1bmN0aW9uIF9wb3NzaWJsZUNvbnN0cnVjdG9yUmV0dXJuKHNlbGYsIGNhbGwpIHsgaWYgKCFzZWxmKSB7IHRocm93IG5ldyBSZWZlcmVuY2VFcnJvcihcInRoaXMgaGFzbid0IGJlZW4gaW5pdGlhbGlzZWQgLSBzdXBlcigpIGhhc24ndCBiZWVuIGNhbGxlZFwiKTsgfSByZXR1cm4gY2FsbCAmJiAodHlwZW9mIGNhbGwgPT09IFwib2JqZWN0XCIgfHwgdHlwZW9mIGNhbGwgPT09IFwiZnVuY3Rpb25cIikgPyBjYWxsIDogc2VsZjsgfVxuXG5mdW5jdGlvbiBfaW5oZXJpdHMoc3ViQ2xhc3MsIHN1cGVyQ2xhc3MpIHsgaWYgKHR5cGVvZiBzdXBlckNsYXNzICE9PSBcImZ1bmN0aW9uXCIgJiYgc3VwZXJDbGFzcyAhPT0gbnVsbCkgeyB0aHJvdyBuZXcgVHlwZUVycm9yKFwiU3VwZXIgZXhwcmVzc2lvbiBtdXN0IGVpdGhlciBiZSBudWxsIG9yIGEgZnVuY3Rpb24sIG5vdCBcIiArIHR5cGVvZiBzdXBlckNsYXNzKTsgfSBzdWJDbGFzcy5wcm90b3R5cGUgPSBPYmplY3QuY3JlYXRlKHN1cGVyQ2xhc3MgJiYgc3VwZXJDbGFzcy5wcm90b3R5cGUsIHsgY29uc3RydWN0b3I6IHsgdmFsdWU6IHN1YkNsYXNzLCBlbnVtZXJhYmxlOiBmYWxzZSwgd3JpdGFibGU6IHRydWUsIGNvbmZpZ3VyYWJsZTogdHJ1ZSB9IH0pOyBpZiAoc3VwZXJDbGFzcykgT2JqZWN0LnNldFByb3RvdHlwZU9mID8gT2JqZWN0LnNldFByb3RvdHlwZU9mKHN1YkNsYXNzLCBzdXBlckNsYXNzKSA6IHN1YkNsYXNzLl9fcHJvdG9fXyA9IHN1cGVyQ2xhc3M7IH1cblxudmFyIEVsZW1lbnQgPSByZXF1aXJlKCcuL0VsZW1lbnQnKTtcblxuLyoqXG4gKi9cblxudmFyIE51bGxFbGVtZW50ID0gZnVuY3Rpb24gKF9FbGVtZW50KSB7XG4gIF9pbmhlcml0cyhOdWxsRWxlbWVudCwgX0VsZW1lbnQpO1xuXG4gIGZ1bmN0aW9uIE51bGxFbGVtZW50KGNvbnRlbnQsIG1ldGEsIGF0dHJpYnV0ZXMpIHtcbiAgICBfY2xhc3NDYWxsQ2hlY2sodGhpcywgTnVsbEVsZW1lbnQpO1xuXG4gICAgdmFyIF90aGlzID0gX3Bvc3NpYmxlQ29uc3RydWN0b3JSZXR1cm4odGhpcywgKE51bGxFbGVtZW50Ll9fcHJvdG9fXyB8fCBPYmplY3QuZ2V0UHJvdG90eXBlT2YoTnVsbEVsZW1lbnQpKS5jYWxsKHRoaXMsIGNvbnRlbnQgfHwgbnVsbCwgbWV0YSwgYXR0cmlidXRlcykpO1xuXG4gICAgX3RoaXMuZWxlbWVudCA9ICdudWxsJztcbiAgICByZXR1cm4gX3RoaXM7XG4gIH1cblxuICBfY3JlYXRlQ2xhc3MoTnVsbEVsZW1lbnQsIFt7XG4gICAga2V5OiAncHJpbWl0aXZlJyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gcHJpbWl0aXZlKCkge1xuICAgICAgcmV0dXJuICdudWxsJztcbiAgICB9XG4gIH0sIHtcbiAgICBrZXk6ICdzZXQnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBzZXQoKSB7XG4gICAgICByZXR1cm4gbmV3IEVycm9yKCdDYW5ub3Qgc2V0IHRoZSB2YWx1ZSBvZiBudWxsJyk7XG4gICAgfVxuICB9XSk7XG5cbiAgcmV0dXJuIE51bGxFbGVtZW50O1xufShFbGVtZW50KTtcblxubW9kdWxlLmV4cG9ydHMgPSBOdWxsRWxlbWVudDsiLCIndXNlIHN0cmljdCc7XG5cbnZhciBfY3JlYXRlQ2xhc3MgPSBmdW5jdGlvbiAoKSB7IGZ1bmN0aW9uIGRlZmluZVByb3BlcnRpZXModGFyZ2V0LCBwcm9wcykgeyBmb3IgKHZhciBpID0gMDsgaSA8IHByb3BzLmxlbmd0aDsgaSsrKSB7IHZhciBkZXNjcmlwdG9yID0gcHJvcHNbaV07IGRlc2NyaXB0b3IuZW51bWVyYWJsZSA9IGRlc2NyaXB0b3IuZW51bWVyYWJsZSB8fCBmYWxzZTsgZGVzY3JpcHRvci5jb25maWd1cmFibGUgPSB0cnVlOyBpZiAoXCJ2YWx1ZVwiIGluIGRlc2NyaXB0b3IpIGRlc2NyaXB0b3Iud3JpdGFibGUgPSB0cnVlOyBPYmplY3QuZGVmaW5lUHJvcGVydHkodGFyZ2V0LCBkZXNjcmlwdG9yLmtleSwgZGVzY3JpcHRvcik7IH0gfSByZXR1cm4gZnVuY3Rpb24gKENvbnN0cnVjdG9yLCBwcm90b1Byb3BzLCBzdGF0aWNQcm9wcykgeyBpZiAocHJvdG9Qcm9wcykgZGVmaW5lUHJvcGVydGllcyhDb25zdHJ1Y3Rvci5wcm90b3R5cGUsIHByb3RvUHJvcHMpOyBpZiAoc3RhdGljUHJvcHMpIGRlZmluZVByb3BlcnRpZXMoQ29uc3RydWN0b3IsIHN0YXRpY1Byb3BzKTsgcmV0dXJuIENvbnN0cnVjdG9yOyB9OyB9KCk7XG5cbmZ1bmN0aW9uIF9jbGFzc0NhbGxDaGVjayhpbnN0YW5jZSwgQ29uc3RydWN0b3IpIHsgaWYgKCEoaW5zdGFuY2UgaW5zdGFuY2VvZiBDb25zdHJ1Y3RvcikpIHsgdGhyb3cgbmV3IFR5cGVFcnJvcihcIkNhbm5vdCBjYWxsIGEgY2xhc3MgYXMgYSBmdW5jdGlvblwiKTsgfSB9XG5cbmZ1bmN0aW9uIF9wb3NzaWJsZUNvbnN0cnVjdG9yUmV0dXJuKHNlbGYsIGNhbGwpIHsgaWYgKCFzZWxmKSB7IHRocm93IG5ldyBSZWZlcmVuY2VFcnJvcihcInRoaXMgaGFzbid0IGJlZW4gaW5pdGlhbGlzZWQgLSBzdXBlcigpIGhhc24ndCBiZWVuIGNhbGxlZFwiKTsgfSByZXR1cm4gY2FsbCAmJiAodHlwZW9mIGNhbGwgPT09IFwib2JqZWN0XCIgfHwgdHlwZW9mIGNhbGwgPT09IFwiZnVuY3Rpb25cIikgPyBjYWxsIDogc2VsZjsgfVxuXG5mdW5jdGlvbiBfaW5oZXJpdHMoc3ViQ2xhc3MsIHN1cGVyQ2xhc3MpIHsgaWYgKHR5cGVvZiBzdXBlckNsYXNzICE9PSBcImZ1bmN0aW9uXCIgJiYgc3VwZXJDbGFzcyAhPT0gbnVsbCkgeyB0aHJvdyBuZXcgVHlwZUVycm9yKFwiU3VwZXIgZXhwcmVzc2lvbiBtdXN0IGVpdGhlciBiZSBudWxsIG9yIGEgZnVuY3Rpb24sIG5vdCBcIiArIHR5cGVvZiBzdXBlckNsYXNzKTsgfSBzdWJDbGFzcy5wcm90b3R5cGUgPSBPYmplY3QuY3JlYXRlKHN1cGVyQ2xhc3MgJiYgc3VwZXJDbGFzcy5wcm90b3R5cGUsIHsgY29uc3RydWN0b3I6IHsgdmFsdWU6IHN1YkNsYXNzLCBlbnVtZXJhYmxlOiBmYWxzZSwgd3JpdGFibGU6IHRydWUsIGNvbmZpZ3VyYWJsZTogdHJ1ZSB9IH0pOyBpZiAoc3VwZXJDbGFzcykgT2JqZWN0LnNldFByb3RvdHlwZU9mID8gT2JqZWN0LnNldFByb3RvdHlwZU9mKHN1YkNsYXNzLCBzdXBlckNsYXNzKSA6IHN1YkNsYXNzLl9fcHJvdG9fXyA9IHN1cGVyQ2xhc3M7IH1cblxudmFyIEVsZW1lbnQgPSByZXF1aXJlKCcuL0VsZW1lbnQnKTtcblxuLyoqXG4gKiBAY2xhc3MgTnVtYmVyRWxlbWVudFxuICpcbiAqIEBwYXJhbSB7bnVtYmVyfSBjb250ZW50XG4gKiBAcGFyYW0gbWV0YVxuICogQHBhcmFtIGF0dHJpYnV0ZXNcbiAqL1xubW9kdWxlLmV4cG9ydHMgPSBmdW5jdGlvbiAoX0VsZW1lbnQpIHtcbiAgX2luaGVyaXRzKE51bWJlckVsZW1lbnQsIF9FbGVtZW50KTtcblxuICBmdW5jdGlvbiBOdW1iZXJFbGVtZW50KGNvbnRlbnQsIG1ldGEsIGF0dHJpYnV0ZXMpIHtcbiAgICBfY2xhc3NDYWxsQ2hlY2sodGhpcywgTnVtYmVyRWxlbWVudCk7XG5cbiAgICB2YXIgX3RoaXMgPSBfcG9zc2libGVDb25zdHJ1Y3RvclJldHVybih0aGlzLCAoTnVtYmVyRWxlbWVudC5fX3Byb3RvX18gfHwgT2JqZWN0LmdldFByb3RvdHlwZU9mKE51bWJlckVsZW1lbnQpKS5jYWxsKHRoaXMsIGNvbnRlbnQsIG1ldGEsIGF0dHJpYnV0ZXMpKTtcblxuICAgIF90aGlzLmVsZW1lbnQgPSAnbnVtYmVyJztcbiAgICByZXR1cm4gX3RoaXM7XG4gIH1cblxuICBfY3JlYXRlQ2xhc3MoTnVtYmVyRWxlbWVudCwgW3tcbiAgICBrZXk6ICdwcmltaXRpdmUnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBwcmltaXRpdmUoKSB7XG4gICAgICByZXR1cm4gJ251bWJlcic7XG4gICAgfVxuICB9XSk7XG5cbiAgcmV0dXJuIE51bWJlckVsZW1lbnQ7XG59KEVsZW1lbnQpOyIsIid1c2Ugc3RyaWN0JztcblxudmFyIF9jcmVhdGVDbGFzcyA9IGZ1bmN0aW9uICgpIHsgZnVuY3Rpb24gZGVmaW5lUHJvcGVydGllcyh0YXJnZXQsIHByb3BzKSB7IGZvciAodmFyIGkgPSAwOyBpIDwgcHJvcHMubGVuZ3RoOyBpKyspIHsgdmFyIGRlc2NyaXB0b3IgPSBwcm9wc1tpXTsgZGVzY3JpcHRvci5lbnVtZXJhYmxlID0gZGVzY3JpcHRvci5lbnVtZXJhYmxlIHx8IGZhbHNlOyBkZXNjcmlwdG9yLmNvbmZpZ3VyYWJsZSA9IHRydWU7IGlmIChcInZhbHVlXCIgaW4gZGVzY3JpcHRvcikgZGVzY3JpcHRvci53cml0YWJsZSA9IHRydWU7IE9iamVjdC5kZWZpbmVQcm9wZXJ0eSh0YXJnZXQsIGRlc2NyaXB0b3Iua2V5LCBkZXNjcmlwdG9yKTsgfSB9IHJldHVybiBmdW5jdGlvbiAoQ29uc3RydWN0b3IsIHByb3RvUHJvcHMsIHN0YXRpY1Byb3BzKSB7IGlmIChwcm90b1Byb3BzKSBkZWZpbmVQcm9wZXJ0aWVzKENvbnN0cnVjdG9yLnByb3RvdHlwZSwgcHJvdG9Qcm9wcyk7IGlmIChzdGF0aWNQcm9wcykgZGVmaW5lUHJvcGVydGllcyhDb25zdHJ1Y3Rvciwgc3RhdGljUHJvcHMpOyByZXR1cm4gQ29uc3RydWN0b3I7IH07IH0oKTtcblxuZnVuY3Rpb24gX2NsYXNzQ2FsbENoZWNrKGluc3RhbmNlLCBDb25zdHJ1Y3RvcikgeyBpZiAoIShpbnN0YW5jZSBpbnN0YW5jZW9mIENvbnN0cnVjdG9yKSkgeyB0aHJvdyBuZXcgVHlwZUVycm9yKFwiQ2Fubm90IGNhbGwgYSBjbGFzcyBhcyBhIGZ1bmN0aW9uXCIpOyB9IH1cblxuZnVuY3Rpb24gX3Bvc3NpYmxlQ29uc3RydWN0b3JSZXR1cm4oc2VsZiwgY2FsbCkgeyBpZiAoIXNlbGYpIHsgdGhyb3cgbmV3IFJlZmVyZW5jZUVycm9yKFwidGhpcyBoYXNuJ3QgYmVlbiBpbml0aWFsaXNlZCAtIHN1cGVyKCkgaGFzbid0IGJlZW4gY2FsbGVkXCIpOyB9IHJldHVybiBjYWxsICYmICh0eXBlb2YgY2FsbCA9PT0gXCJvYmplY3RcIiB8fCB0eXBlb2YgY2FsbCA9PT0gXCJmdW5jdGlvblwiKSA/IGNhbGwgOiBzZWxmOyB9XG5cbmZ1bmN0aW9uIF9pbmhlcml0cyhzdWJDbGFzcywgc3VwZXJDbGFzcykgeyBpZiAodHlwZW9mIHN1cGVyQ2xhc3MgIT09IFwiZnVuY3Rpb25cIiAmJiBzdXBlckNsYXNzICE9PSBudWxsKSB7IHRocm93IG5ldyBUeXBlRXJyb3IoXCJTdXBlciBleHByZXNzaW9uIG11c3QgZWl0aGVyIGJlIG51bGwgb3IgYSBmdW5jdGlvbiwgbm90IFwiICsgdHlwZW9mIHN1cGVyQ2xhc3MpOyB9IHN1YkNsYXNzLnByb3RvdHlwZSA9IE9iamVjdC5jcmVhdGUoc3VwZXJDbGFzcyAmJiBzdXBlckNsYXNzLnByb3RvdHlwZSwgeyBjb25zdHJ1Y3RvcjogeyB2YWx1ZTogc3ViQ2xhc3MsIGVudW1lcmFibGU6IGZhbHNlLCB3cml0YWJsZTogdHJ1ZSwgY29uZmlndXJhYmxlOiB0cnVlIH0gfSk7IGlmIChzdXBlckNsYXNzKSBPYmplY3Quc2V0UHJvdG90eXBlT2YgPyBPYmplY3Quc2V0UHJvdG90eXBlT2Yoc3ViQ2xhc3MsIHN1cGVyQ2xhc3MpIDogc3ViQ2xhc3MuX19wcm90b19fID0gc3VwZXJDbGFzczsgfVxuXG52YXIgbmVnYXRlID0gcmVxdWlyZSgnbG9kYXNoL25lZ2F0ZScpO1xudmFyIGlzT2JqZWN0ID0gcmVxdWlyZSgnbG9kYXNoL2lzT2JqZWN0Jyk7XG5cbnZhciBBcnJheUVsZW1lbnQgPSByZXF1aXJlKCcuL0FycmF5RWxlbWVudCcpO1xudmFyIE1lbWJlckVsZW1lbnQgPSByZXF1aXJlKCcuL01lbWJlckVsZW1lbnQnKTtcbnZhciBPYmplY3RTbGljZSA9IHJlcXVpcmUoJy4uL09iamVjdFNsaWNlJyk7XG5cbi8qKlxuICogQGNsYXNzXG4gKlxuICogQHBhcmFtIGNvbnRlbnRcbiAqIEBwYXJhbSBtZXRhXG4gKiBAcGFyYW0gYXR0cmlidXRlc1xuICovXG5cbnZhciBPYmplY3RFbGVtZW50ID0gZnVuY3Rpb24gKF9BcnJheUVsZW1lbnQpIHtcbiAgX2luaGVyaXRzKE9iamVjdEVsZW1lbnQsIF9BcnJheUVsZW1lbnQpO1xuXG4gIGZ1bmN0aW9uIE9iamVjdEVsZW1lbnQoY29udGVudCwgbWV0YSwgYXR0cmlidXRlcykge1xuICAgIF9jbGFzc0NhbGxDaGVjayh0aGlzLCBPYmplY3RFbGVtZW50KTtcblxuICAgIHZhciBfdGhpcyA9IF9wb3NzaWJsZUNvbnN0cnVjdG9yUmV0dXJuKHRoaXMsIChPYmplY3RFbGVtZW50Ll9fcHJvdG9fXyB8fCBPYmplY3QuZ2V0UHJvdG90eXBlT2YoT2JqZWN0RWxlbWVudCkpLmNhbGwodGhpcywgY29udGVudCB8fCBbXSwgbWV0YSwgYXR0cmlidXRlcykpO1xuXG4gICAgX3RoaXMuZWxlbWVudCA9ICdvYmplY3QnO1xuICAgIHJldHVybiBfdGhpcztcbiAgfVxuXG4gIF9jcmVhdGVDbGFzcyhPYmplY3RFbGVtZW50LCBbe1xuICAgIGtleTogJ3ByaW1pdGl2ZScsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIHByaW1pdGl2ZSgpIHtcbiAgICAgIHJldHVybiAnb2JqZWN0JztcbiAgICB9XG4gIH0sIHtcbiAgICBrZXk6ICd0b1ZhbHVlJyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gdG9WYWx1ZSgpIHtcbiAgICAgIHJldHVybiB0aGlzLmNvbnRlbnQucmVkdWNlKGZ1bmN0aW9uIChyZXN1bHRzLCBlbCkge1xuICAgICAgICByZXN1bHRzW2VsLmtleS50b1ZhbHVlKCldID0gZWwudmFsdWUgPyBlbC52YWx1ZS50b1ZhbHVlKCkgOiB1bmRlZmluZWQ7XG4gICAgICAgIHJldHVybiByZXN1bHRzO1xuICAgICAgfSwge30pO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEBwYXJhbSBrZXlcbiAgICAgKiBAcmV0dXJucyB7RWxlbWVudH1cbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAnZ2V0JyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gZ2V0KG5hbWUpIHtcbiAgICAgIHZhciBtZW1iZXIgPSB0aGlzLmdldE1lbWJlcihuYW1lKTtcblxuICAgICAgaWYgKG1lbWJlcikge1xuICAgICAgICByZXR1cm4gbWVtYmVyLnZhbHVlO1xuICAgICAgfVxuXG4gICAgICByZXR1cm4gdW5kZWZpbmVkO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEBwYXJhbSBrZXlcbiAgICAgKiBAcmV0dXJucyB7TWVtYmVyRWxlbWVudH1cbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAnZ2V0TWVtYmVyJyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gZ2V0TWVtYmVyKG5hbWUpIHtcbiAgICAgIGlmIChuYW1lID09PSB1bmRlZmluZWQpIHtcbiAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgIH1cblxuICAgICAgcmV0dXJuIHRoaXMuY29udGVudC5maW5kKGZ1bmN0aW9uIChlbGVtZW50KSB7XG4gICAgICAgIHJldHVybiBlbGVtZW50LmtleS50b1ZhbHVlKCkgPT09IG5hbWU7XG4gICAgICB9KTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBAcGFyYW0ga2V5XG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ3JlbW92ZScsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIHJlbW92ZShuYW1lKSB7XG4gICAgICB2YXIgcmVtb3ZlZCA9IG51bGw7XG5cbiAgICAgIHRoaXMuY29udGVudCA9IHRoaXMuY29udGVudC5maWx0ZXIoZnVuY3Rpb24gKGl0ZW0pIHtcbiAgICAgICAgaWYgKGl0ZW0ua2V5LnRvVmFsdWUoKSA9PT0gbmFtZSkge1xuICAgICAgICAgIHJlbW92ZWQgPSBpdGVtO1xuICAgICAgICAgIHJldHVybiBmYWxzZTtcbiAgICAgICAgfVxuXG4gICAgICAgIHJldHVybiB0cnVlO1xuICAgICAgfSk7XG5cbiAgICAgIHJldHVybiByZW1vdmVkO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEBwYXJhbSBrZXlcbiAgICAgKiBAcmV0dXJucyB7RWxlbWVudH1cbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAnZ2V0S2V5JyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gZ2V0S2V5KG5hbWUpIHtcbiAgICAgIHZhciBtZW1iZXIgPSB0aGlzLmdldE1lbWJlcihuYW1lKTtcblxuICAgICAgaWYgKG1lbWJlcikge1xuICAgICAgICByZXR1cm4gbWVtYmVyLmtleTtcbiAgICAgIH1cblxuICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBTZXQgYWxsb3dzIGVpdGhlciBhIGtleS92YWx1ZSBwYWlyIHRvIGJlIGdpdmVuIG9yIGFuIG9iamVjdFxuICAgICAqIElmIGFuIG9iamVjdCBpcyBnaXZlbiwgZWFjaCBrZXkgaXMgc2V0IHRvIGl0cyByZXNwZWN0aXZlIHZhbHVlXG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ3NldCcsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIHNldChrZXlPck9iamVjdCwgdmFsdWUpIHtcbiAgICAgIHZhciBfdGhpczIgPSB0aGlzO1xuXG4gICAgICBpZiAoaXNPYmplY3Qoa2V5T3JPYmplY3QpKSB7XG4gICAgICAgIE9iamVjdC5rZXlzKGtleU9yT2JqZWN0KS5mb3JFYWNoKGZ1bmN0aW9uIChvYmplY3RLZXkpIHtcbiAgICAgICAgICBfdGhpczIuc2V0KG9iamVjdEtleSwga2V5T3JPYmplY3Rbb2JqZWN0S2V5XSk7XG4gICAgICAgIH0pO1xuXG4gICAgICAgIHJldHVybiB0aGlzO1xuICAgICAgfVxuXG4gICAgICAvLyBTdG9yZSBhcyBrZXkgZm9yIGNsYXJpdHlcbiAgICAgIHZhciBrZXkgPSBrZXlPck9iamVjdDtcbiAgICAgIHZhciBtZW1iZXIgPSB0aGlzLmdldE1lbWJlcihrZXkpO1xuXG4gICAgICBpZiAobWVtYmVyKSB7XG4gICAgICAgIG1lbWJlci52YWx1ZSA9IHZhbHVlO1xuICAgICAgfSBlbHNlIHtcbiAgICAgICAgdGhpcy5jb250ZW50LnB1c2gobmV3IE1lbWJlckVsZW1lbnQoa2V5LCB2YWx1ZSkpO1xuICAgICAgfVxuXG4gICAgICByZXR1cm4gdGhpcztcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAna2V5cycsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIGtleXMoKSB7XG4gICAgICByZXR1cm4gdGhpcy5jb250ZW50Lm1hcChmdW5jdGlvbiAoaXRlbSkge1xuICAgICAgICByZXR1cm4gaXRlbS5rZXkudG9WYWx1ZSgpO1xuICAgICAgfSk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ3ZhbHVlcycsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIHZhbHVlcygpIHtcbiAgICAgIHJldHVybiB0aGlzLmNvbnRlbnQubWFwKGZ1bmN0aW9uIChpdGVtKSB7XG4gICAgICAgIHJldHVybiBpdGVtLnZhbHVlLnRvVmFsdWUoKTtcbiAgICAgIH0pO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEByZXR1cm5zIHtib29sZWFufVxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdoYXNLZXknLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBoYXNLZXkodmFsdWUpIHtcbiAgICAgIHJldHVybiB0aGlzLmNvbnRlbnQuc29tZShmdW5jdGlvbiAobWVtYmVyKSB7XG4gICAgICAgIHJldHVybiBtZW1iZXIua2V5LmVxdWFscyh2YWx1ZSk7XG4gICAgICB9KTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBAcmV0dXJucyB7YXJyYXl9XG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ2l0ZW1zJyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gaXRlbXMoKSB7XG4gICAgICByZXR1cm4gdGhpcy5jb250ZW50Lm1hcChmdW5jdGlvbiAoaXRlbSkge1xuICAgICAgICByZXR1cm4gW2l0ZW0ua2V5LnRvVmFsdWUoKSwgaXRlbS52YWx1ZS50b1ZhbHVlKCldO1xuICAgICAgfSk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQHBhcmFtIGNhbGxiYWNrXG4gICAgICogQHBhcmFtIHRoaXNBcmcgLSBWYWx1ZSB0byB1c2UgYXMgdGhpcyAoaS5lIHRoZSByZWZlcmVuY2UgT2JqZWN0KSB3aGVuIGV4ZWN1dGluZyBjYWxsYmFja1xuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdtYXAnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBtYXAoY2FsbGJhY2ssIHRoaXNBcmcpIHtcbiAgICAgIHJldHVybiB0aGlzLmNvbnRlbnQubWFwKGZ1bmN0aW9uIChpdGVtKSB7XG4gICAgICAgIHJldHVybiBjYWxsYmFjay5iaW5kKHRoaXNBcmcpKGl0ZW0udmFsdWUsIGl0ZW0ua2V5LCBpdGVtKTtcbiAgICAgIH0pO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIFJldHVybnMgYW4gYXJyYXkgY29udGFpbmluZyB0aGUgdHJ1dGh5IHJlc3VsdHMgb2YgY2FsbGluZyB0aGUgZ2l2ZW4gdHJhbnNmb3JtYXRpb24gd2l0aCBlYWNoIGVsZW1lbnQgb2YgdGhpcyBzZXF1ZW5jZVxuICAgICAqIEBwYXJhbSB0cmFuc2Zvcm0gLSBBIGNsb3N1cmUgdGhhdCBhY2NlcHRzIHRoZSB2YWx1ZSwga2V5IGFuZCBtZW1iZXIgZWxlbWVudCBvZiB0aGlzIG9iamVjdCBhcyBpdHMgYXJndW1lbnQgYW5kIHJldHVybnMgYW4gb3B0aW9uYWwgdmFsdWUuXG4gICAgICogQHBhcmFtIHRoaXNBcmcgLSBWYWx1ZSB0byB1c2UgYXMgdGhpcyAoaS5lIHRoZSByZWZlcmVuY2UgT2JqZWN0KSB3aGVuIGV4ZWN1dGluZyBjYWxsYmFja1xuICAgICAqIEByZXR1cm5zIEFuIGFycmF5IG9mIHRoZSBub24tdW5kZWZpbmVkIHJlc3VsdHMgb2YgY2FsbGluZyB0cmFuc2Zvcm0gd2l0aCBlYWNoIGVsZW1lbnQgb2YgdGhlIGFycmF5XG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ2NvbXBhY3RNYXAnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBjb21wYWN0TWFwKGNhbGxiYWNrLCB0aGlzQXJnKSB7XG4gICAgICB2YXIgcmVzdWx0cyA9IFtdO1xuXG4gICAgICB0aGlzLmZvckVhY2goZnVuY3Rpb24gKHZhbHVlLCBrZXksIG1lbWJlcikge1xuICAgICAgICB2YXIgcmVzdWx0ID0gY2FsbGJhY2suYmluZCh0aGlzQXJnKSh2YWx1ZSwga2V5LCBtZW1iZXIpO1xuXG4gICAgICAgIGlmIChyZXN1bHQpIHtcbiAgICAgICAgICByZXN1bHRzLnB1c2gocmVzdWx0KTtcbiAgICAgICAgfVxuICAgICAgfSk7XG5cbiAgICAgIHJldHVybiByZXN1bHRzO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEBwYXJhbSBjYWxsYmFja1xuICAgICAqIEBwYXJhbSB0aGlzQXJnIC0gVmFsdWUgdG8gdXNlIGFzIHRoaXMgKGkuZSB0aGUgcmVmZXJlbmNlIE9iamVjdCkgd2hlbiBleGVjdXRpbmcgY2FsbGJhY2tcbiAgICAgKlxuICAgICAqIEByZXR1cm5zIHtPYmplY3RTbGljZX1cbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAnZmlsdGVyJyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gZmlsdGVyKGNhbGxiYWNrLCB0aGlzQXJnKSB7XG4gICAgICByZXR1cm4gbmV3IE9iamVjdFNsaWNlKHRoaXMuY29udGVudCkuZmlsdGVyKGNhbGxiYWNrLCB0aGlzQXJnKTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBAcGFyYW0gY2FsbGJhY2tcbiAgICAgKiBAcGFyYW0gdGhpc0FyZyAtIFZhbHVlIHRvIHVzZSBhcyB0aGlzIChpLmUgdGhlIHJlZmVyZW5jZSBPYmplY3QpIHdoZW4gZXhlY3V0aW5nIGNhbGxiYWNrXG4gICAgICpcbiAgICAgKiBAcmV0dXJucyB7T2JqZWN0U2xpY2V9XG4gICAgICpcbiAgICAgKiBAbWVtYmVyb2YgT2JqZWN0RWxlbWVudC5wcm90b3R5cGVcbiAgICAgKi9cblxuICB9LCB7XG4gICAga2V5OiAncmVqZWN0JyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gcmVqZWN0KGNhbGxiYWNrLCB0aGlzQXJnKSB7XG4gICAgICByZXR1cm4gdGhpcy5maWx0ZXIobmVnYXRlKGNhbGxiYWNrKSwgdGhpc0FyZyk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQHBhcmFtIGNhbGxiYWNrXG4gICAgICogQHBhcmFtIHRoaXNBcmcgLSBWYWx1ZSB0byB1c2UgYXMgdGhpcyAoaS5lIHRoZSByZWZlcmVuY2UgT2JqZWN0KSB3aGVuIGV4ZWN1dGluZyBjYWxsYmFja1xuICAgICAqXG4gICAgICogQG1lbWJlcm9mIE9iamVjdEVsZW1lbnQucHJvdG90eXBlXG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ2ZvckVhY2gnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBmb3JFYWNoKGNhbGxiYWNrLCB0aGlzQXJnKSB7XG4gICAgICByZXR1cm4gdGhpcy5jb250ZW50LmZvckVhY2goZnVuY3Rpb24gKGl0ZW0pIHtcbiAgICAgICAgcmV0dXJuIGNhbGxiYWNrLmJpbmQodGhpc0FyZykoaXRlbS52YWx1ZSwgaXRlbS5rZXksIGl0ZW0pO1xuICAgICAgfSk7XG4gICAgfVxuICB9XSk7XG5cbiAgcmV0dXJuIE9iamVjdEVsZW1lbnQ7XG59KEFycmF5RWxlbWVudCk7XG5cbm1vZHVsZS5leHBvcnRzID0gT2JqZWN0RWxlbWVudDsiLCIndXNlIHN0cmljdCc7XG5cbnZhciBfY3JlYXRlQ2xhc3MgPSBmdW5jdGlvbiAoKSB7IGZ1bmN0aW9uIGRlZmluZVByb3BlcnRpZXModGFyZ2V0LCBwcm9wcykgeyBmb3IgKHZhciBpID0gMDsgaSA8IHByb3BzLmxlbmd0aDsgaSsrKSB7IHZhciBkZXNjcmlwdG9yID0gcHJvcHNbaV07IGRlc2NyaXB0b3IuZW51bWVyYWJsZSA9IGRlc2NyaXB0b3IuZW51bWVyYWJsZSB8fCBmYWxzZTsgZGVzY3JpcHRvci5jb25maWd1cmFibGUgPSB0cnVlOyBpZiAoXCJ2YWx1ZVwiIGluIGRlc2NyaXB0b3IpIGRlc2NyaXB0b3Iud3JpdGFibGUgPSB0cnVlOyBPYmplY3QuZGVmaW5lUHJvcGVydHkodGFyZ2V0LCBkZXNjcmlwdG9yLmtleSwgZGVzY3JpcHRvcik7IH0gfSByZXR1cm4gZnVuY3Rpb24gKENvbnN0cnVjdG9yLCBwcm90b1Byb3BzLCBzdGF0aWNQcm9wcykgeyBpZiAocHJvdG9Qcm9wcykgZGVmaW5lUHJvcGVydGllcyhDb25zdHJ1Y3Rvci5wcm90b3R5cGUsIHByb3RvUHJvcHMpOyBpZiAoc3RhdGljUHJvcHMpIGRlZmluZVByb3BlcnRpZXMoQ29uc3RydWN0b3IsIHN0YXRpY1Byb3BzKTsgcmV0dXJuIENvbnN0cnVjdG9yOyB9OyB9KCk7XG5cbmZ1bmN0aW9uIF9jbGFzc0NhbGxDaGVjayhpbnN0YW5jZSwgQ29uc3RydWN0b3IpIHsgaWYgKCEoaW5zdGFuY2UgaW5zdGFuY2VvZiBDb25zdHJ1Y3RvcikpIHsgdGhyb3cgbmV3IFR5cGVFcnJvcihcIkNhbm5vdCBjYWxsIGEgY2xhc3MgYXMgYSBmdW5jdGlvblwiKTsgfSB9XG5cbmZ1bmN0aW9uIF9wb3NzaWJsZUNvbnN0cnVjdG9yUmV0dXJuKHNlbGYsIGNhbGwpIHsgaWYgKCFzZWxmKSB7IHRocm93IG5ldyBSZWZlcmVuY2VFcnJvcihcInRoaXMgaGFzbid0IGJlZW4gaW5pdGlhbGlzZWQgLSBzdXBlcigpIGhhc24ndCBiZWVuIGNhbGxlZFwiKTsgfSByZXR1cm4gY2FsbCAmJiAodHlwZW9mIGNhbGwgPT09IFwib2JqZWN0XCIgfHwgdHlwZW9mIGNhbGwgPT09IFwiZnVuY3Rpb25cIikgPyBjYWxsIDogc2VsZjsgfVxuXG5mdW5jdGlvbiBfaW5oZXJpdHMoc3ViQ2xhc3MsIHN1cGVyQ2xhc3MpIHsgaWYgKHR5cGVvZiBzdXBlckNsYXNzICE9PSBcImZ1bmN0aW9uXCIgJiYgc3VwZXJDbGFzcyAhPT0gbnVsbCkgeyB0aHJvdyBuZXcgVHlwZUVycm9yKFwiU3VwZXIgZXhwcmVzc2lvbiBtdXN0IGVpdGhlciBiZSBudWxsIG9yIGEgZnVuY3Rpb24sIG5vdCBcIiArIHR5cGVvZiBzdXBlckNsYXNzKTsgfSBzdWJDbGFzcy5wcm90b3R5cGUgPSBPYmplY3QuY3JlYXRlKHN1cGVyQ2xhc3MgJiYgc3VwZXJDbGFzcy5wcm90b3R5cGUsIHsgY29uc3RydWN0b3I6IHsgdmFsdWU6IHN1YkNsYXNzLCBlbnVtZXJhYmxlOiBmYWxzZSwgd3JpdGFibGU6IHRydWUsIGNvbmZpZ3VyYWJsZTogdHJ1ZSB9IH0pOyBpZiAoc3VwZXJDbGFzcykgT2JqZWN0LnNldFByb3RvdHlwZU9mID8gT2JqZWN0LnNldFByb3RvdHlwZU9mKHN1YkNsYXNzLCBzdXBlckNsYXNzKSA6IHN1YkNsYXNzLl9fcHJvdG9fXyA9IHN1cGVyQ2xhc3M7IH1cblxudmFyIEVsZW1lbnQgPSByZXF1aXJlKCcuL0VsZW1lbnQnKTtcblxuLyoqXG4gKiBAY2xhc3MgU3RyaW5nRWxlbWVudFxuICpcbiAqIEBwYXJhbSB7c3RyaW5nfSBjb250ZW50XG4gKiBAcGFyYW0gbWV0YVxuICogQHBhcmFtIGF0dHJpYnV0ZXNcbiAqL1xubW9kdWxlLmV4cG9ydHMgPSBmdW5jdGlvbiAoX0VsZW1lbnQpIHtcbiAgX2luaGVyaXRzKFN0cmluZ0VsZW1lbnQsIF9FbGVtZW50KTtcblxuICBmdW5jdGlvbiBTdHJpbmdFbGVtZW50KGNvbnRlbnQsIG1ldGEsIGF0dHJpYnV0ZXMpIHtcbiAgICBfY2xhc3NDYWxsQ2hlY2sodGhpcywgU3RyaW5nRWxlbWVudCk7XG5cbiAgICB2YXIgX3RoaXMgPSBfcG9zc2libGVDb25zdHJ1Y3RvclJldHVybih0aGlzLCAoU3RyaW5nRWxlbWVudC5fX3Byb3RvX18gfHwgT2JqZWN0LmdldFByb3RvdHlwZU9mKFN0cmluZ0VsZW1lbnQpKS5jYWxsKHRoaXMsIGNvbnRlbnQsIG1ldGEsIGF0dHJpYnV0ZXMpKTtcblxuICAgIF90aGlzLmVsZW1lbnQgPSAnc3RyaW5nJztcbiAgICByZXR1cm4gX3RoaXM7XG4gIH1cblxuICBfY3JlYXRlQ2xhc3MoU3RyaW5nRWxlbWVudCwgW3tcbiAgICBrZXk6ICdwcmltaXRpdmUnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBwcmltaXRpdmUoKSB7XG4gICAgICByZXR1cm4gJ3N0cmluZyc7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogVGhlIGxlbmd0aCBvZiB0aGUgc3RyaW5nLlxuICAgICAqIEB0eXBlIG51bWJlclxuICAgICAqL1xuXG4gIH0sIHtcbiAgICBrZXk6ICdsZW5ndGgnLFxuICAgIGdldDogZnVuY3Rpb24gZ2V0KCkge1xuICAgICAgcmV0dXJuIHRoaXMuY29udGVudC5sZW5ndGg7XG4gICAgfVxuICB9XSk7XG5cbiAgcmV0dXJuIFN0cmluZ0VsZW1lbnQ7XG59KEVsZW1lbnQpOyIsIid1c2Ugc3RyaWN0JztcblxudmFyIF9zbGljZWRUb0FycmF5ID0gZnVuY3Rpb24gKCkgeyBmdW5jdGlvbiBzbGljZUl0ZXJhdG9yKGFyciwgaSkgeyB2YXIgX2FyciA9IFtdOyB2YXIgX24gPSB0cnVlOyB2YXIgX2QgPSBmYWxzZTsgdmFyIF9lID0gdW5kZWZpbmVkOyB0cnkgeyBmb3IgKHZhciBfaSA9IGFycltTeW1ib2wuaXRlcmF0b3JdKCksIF9zOyAhKF9uID0gKF9zID0gX2kubmV4dCgpKS5kb25lKTsgX24gPSB0cnVlKSB7IF9hcnIucHVzaChfcy52YWx1ZSk7IGlmIChpICYmIF9hcnIubGVuZ3RoID09PSBpKSBicmVhazsgfSB9IGNhdGNoIChlcnIpIHsgX2QgPSB0cnVlOyBfZSA9IGVycjsgfSBmaW5hbGx5IHsgdHJ5IHsgaWYgKCFfbiAmJiBfaVtcInJldHVyblwiXSkgX2lbXCJyZXR1cm5cIl0oKTsgfSBmaW5hbGx5IHsgaWYgKF9kKSB0aHJvdyBfZTsgfSB9IHJldHVybiBfYXJyOyB9IHJldHVybiBmdW5jdGlvbiAoYXJyLCBpKSB7IGlmIChBcnJheS5pc0FycmF5KGFycikpIHsgcmV0dXJuIGFycjsgfSBlbHNlIGlmIChTeW1ib2wuaXRlcmF0b3IgaW4gT2JqZWN0KGFycikpIHsgcmV0dXJuIHNsaWNlSXRlcmF0b3IoYXJyLCBpKTsgfSBlbHNlIHsgdGhyb3cgbmV3IFR5cGVFcnJvcihcIkludmFsaWQgYXR0ZW1wdCB0byBkZXN0cnVjdHVyZSBub24taXRlcmFibGUgaW5zdGFuY2VcIik7IH0gfTsgfSgpO1xuXG52YXIgX2NyZWF0ZUNsYXNzID0gZnVuY3Rpb24gKCkgeyBmdW5jdGlvbiBkZWZpbmVQcm9wZXJ0aWVzKHRhcmdldCwgcHJvcHMpIHsgZm9yICh2YXIgaSA9IDA7IGkgPCBwcm9wcy5sZW5ndGg7IGkrKykgeyB2YXIgZGVzY3JpcHRvciA9IHByb3BzW2ldOyBkZXNjcmlwdG9yLmVudW1lcmFibGUgPSBkZXNjcmlwdG9yLmVudW1lcmFibGUgfHwgZmFsc2U7IGRlc2NyaXB0b3IuY29uZmlndXJhYmxlID0gdHJ1ZTsgaWYgKFwidmFsdWVcIiBpbiBkZXNjcmlwdG9yKSBkZXNjcmlwdG9yLndyaXRhYmxlID0gdHJ1ZTsgT2JqZWN0LmRlZmluZVByb3BlcnR5KHRhcmdldCwgZGVzY3JpcHRvci5rZXksIGRlc2NyaXB0b3IpOyB9IH0gcmV0dXJuIGZ1bmN0aW9uIChDb25zdHJ1Y3RvciwgcHJvdG9Qcm9wcywgc3RhdGljUHJvcHMpIHsgaWYgKHByb3RvUHJvcHMpIGRlZmluZVByb3BlcnRpZXMoQ29uc3RydWN0b3IucHJvdG90eXBlLCBwcm90b1Byb3BzKTsgaWYgKHN0YXRpY1Byb3BzKSBkZWZpbmVQcm9wZXJ0aWVzKENvbnN0cnVjdG9yLCBzdGF0aWNQcm9wcyk7IHJldHVybiBDb25zdHJ1Y3RvcjsgfTsgfSgpO1xuXG5mdW5jdGlvbiBfY2xhc3NDYWxsQ2hlY2soaW5zdGFuY2UsIENvbnN0cnVjdG9yKSB7IGlmICghKGluc3RhbmNlIGluc3RhbmNlb2YgQ29uc3RydWN0b3IpKSB7IHRocm93IG5ldyBUeXBlRXJyb3IoXCJDYW5ub3QgY2FsbCBhIGNsYXNzIGFzIGEgZnVuY3Rpb25cIik7IH0gfVxuXG5mdW5jdGlvbiBfcG9zc2libGVDb25zdHJ1Y3RvclJldHVybihzZWxmLCBjYWxsKSB7IGlmICghc2VsZikgeyB0aHJvdyBuZXcgUmVmZXJlbmNlRXJyb3IoXCJ0aGlzIGhhc24ndCBiZWVuIGluaXRpYWxpc2VkIC0gc3VwZXIoKSBoYXNuJ3QgYmVlbiBjYWxsZWRcIik7IH0gcmV0dXJuIGNhbGwgJiYgKHR5cGVvZiBjYWxsID09PSBcIm9iamVjdFwiIHx8IHR5cGVvZiBjYWxsID09PSBcImZ1bmN0aW9uXCIpID8gY2FsbCA6IHNlbGY7IH1cblxuZnVuY3Rpb24gX2luaGVyaXRzKHN1YkNsYXNzLCBzdXBlckNsYXNzKSB7IGlmICh0eXBlb2Ygc3VwZXJDbGFzcyAhPT0gXCJmdW5jdGlvblwiICYmIHN1cGVyQ2xhc3MgIT09IG51bGwpIHsgdGhyb3cgbmV3IFR5cGVFcnJvcihcIlN1cGVyIGV4cHJlc3Npb24gbXVzdCBlaXRoZXIgYmUgbnVsbCBvciBhIGZ1bmN0aW9uLCBub3QgXCIgKyB0eXBlb2Ygc3VwZXJDbGFzcyk7IH0gc3ViQ2xhc3MucHJvdG90eXBlID0gT2JqZWN0LmNyZWF0ZShzdXBlckNsYXNzICYmIHN1cGVyQ2xhc3MucHJvdG90eXBlLCB7IGNvbnN0cnVjdG9yOiB7IHZhbHVlOiBzdWJDbGFzcywgZW51bWVyYWJsZTogZmFsc2UsIHdyaXRhYmxlOiB0cnVlLCBjb25maWd1cmFibGU6IHRydWUgfSB9KTsgaWYgKHN1cGVyQ2xhc3MpIE9iamVjdC5zZXRQcm90b3R5cGVPZiA/IE9iamVjdC5zZXRQcm90b3R5cGVPZihzdWJDbGFzcywgc3VwZXJDbGFzcykgOiBzdWJDbGFzcy5fX3Byb3RvX18gPSBzdXBlckNsYXNzOyB9XG5cbnZhciBKU09OU2VyaWFsaXNlciA9IHJlcXVpcmUoJy4vSlNPTlNlcmlhbGlzZXInKTtcblxubW9kdWxlLmV4cG9ydHMgPSBmdW5jdGlvbiAoX0pTT05TZXJpYWxpc2VyKSB7XG4gIF9pbmhlcml0cyhKU09OMDZTZXJpYWxpc2VyLCBfSlNPTlNlcmlhbGlzZXIpO1xuXG4gIGZ1bmN0aW9uIEpTT04wNlNlcmlhbGlzZXIoKSB7XG4gICAgX2NsYXNzQ2FsbENoZWNrKHRoaXMsIEpTT04wNlNlcmlhbGlzZXIpO1xuXG4gICAgcmV0dXJuIF9wb3NzaWJsZUNvbnN0cnVjdG9yUmV0dXJuKHRoaXMsIChKU09OMDZTZXJpYWxpc2VyLl9fcHJvdG9fXyB8fCBPYmplY3QuZ2V0UHJvdG90eXBlT2YoSlNPTjA2U2VyaWFsaXNlcikpLmFwcGx5KHRoaXMsIGFyZ3VtZW50cykpO1xuICB9XG5cbiAgX2NyZWF0ZUNsYXNzKEpTT04wNlNlcmlhbGlzZXIsIFt7XG4gICAga2V5OiAnc2VyaWFsaXNlJyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gc2VyaWFsaXNlKGVsZW1lbnQpIHtcbiAgICAgIGlmICghKGVsZW1lbnQgaW5zdGFuY2VvZiB0aGlzLm5hbWVzcGFjZS5lbGVtZW50cy5FbGVtZW50KSkge1xuICAgICAgICB0aHJvdyBuZXcgVHlwZUVycm9yKCdHaXZlbiBlbGVtZW50IGAnICsgZWxlbWVudCArICdgIGlzIG5vdCBhbiBFbGVtZW50IGluc3RhbmNlJyk7XG4gICAgICB9XG5cbiAgICAgIHZhciB2YXJpYWJsZSA9IHZvaWQgMDtcbiAgICAgIGlmIChlbGVtZW50Ll9hdHRyaWJ1dGVzICYmIGVsZW1lbnQuYXR0cmlidXRlcy5nZXQoJ3ZhcmlhYmxlJykpIHtcbiAgICAgICAgdmFyaWFibGUgPSBlbGVtZW50LmF0dHJpYnV0ZXMuZ2V0KCd2YXJpYWJsZScpO1xuICAgICAgfVxuXG4gICAgICB2YXIgcGF5bG9hZCA9IHtcbiAgICAgICAgZWxlbWVudDogZWxlbWVudC5lbGVtZW50XG4gICAgICB9O1xuXG4gICAgICBpZiAoZWxlbWVudC5fbWV0YSAmJiBlbGVtZW50Ll9tZXRhLmxlbmd0aCA+IDApIHtcbiAgICAgICAgcGF5bG9hZC5tZXRhID0gdGhpcy5zZXJpYWxpc2VPYmplY3QoZWxlbWVudC5tZXRhKTtcbiAgICAgIH1cblxuICAgICAgdmFyIGlzRW51bSA9IGVsZW1lbnQuZWxlbWVudCA9PT0gJ2VudW0nIHx8IGVsZW1lbnQuYXR0cmlidXRlcy5rZXlzKCkuaW5kZXhPZignZW51bWVyYXRpb25zJykgIT09IC0xO1xuXG4gICAgICBpZiAoaXNFbnVtKSB7XG4gICAgICAgIHZhciBhdHRyaWJ1dGVzID0gdGhpcy5lbnVtU2VyaWFsaXNlQXR0cmlidXRlcyhlbGVtZW50KTtcblxuICAgICAgICBpZiAoYXR0cmlidXRlcykge1xuICAgICAgICAgIHBheWxvYWQuYXR0cmlidXRlcyA9IGF0dHJpYnV0ZXM7XG4gICAgICAgIH1cbiAgICAgIH0gZWxzZSBpZiAoZWxlbWVudC5fYXR0cmlidXRlcyAmJiBlbGVtZW50Ll9hdHRyaWJ1dGVzLmxlbmd0aCA+IDApIHtcbiAgICAgICAgdmFyIF9hdHRyaWJ1dGVzID0gZWxlbWVudC5hdHRyaWJ1dGVzO1xuXG4gICAgICAgIC8vIE1ldGEgYXR0cmlidXRlIHdhcyByZW5hbWVkIHRvIG1ldGFkYXRhXG5cbiAgICAgICAgaWYgKF9hdHRyaWJ1dGVzLmdldCgnbWV0YWRhdGEnKSkge1xuICAgICAgICAgIF9hdHRyaWJ1dGVzID0gX2F0dHJpYnV0ZXMuY2xvbmUoKTtcbiAgICAgICAgICBfYXR0cmlidXRlcy5zZXQoJ21ldGEnLCBfYXR0cmlidXRlcy5nZXQoJ21ldGFkYXRhJykpO1xuICAgICAgICAgIF9hdHRyaWJ1dGVzLnJlbW92ZSgnbWV0YWRhdGEnKTtcbiAgICAgICAgfVxuXG4gICAgICAgIGlmIChlbGVtZW50LmVsZW1lbnQgPT09ICdtZW1iZXInICYmIHZhcmlhYmxlKSB7XG4gICAgICAgICAgX2F0dHJpYnV0ZXMgPSBfYXR0cmlidXRlcy5jbG9uZSgpO1xuICAgICAgICAgIF9hdHRyaWJ1dGVzLnJlbW92ZSgndmFyaWFibGUnKTtcbiAgICAgICAgfVxuXG4gICAgICAgIGlmIChfYXR0cmlidXRlcy5sZW5ndGggPiAwKSB7XG4gICAgICAgICAgcGF5bG9hZC5hdHRyaWJ1dGVzID0gdGhpcy5zZXJpYWxpc2VPYmplY3QoX2F0dHJpYnV0ZXMpO1xuICAgICAgICB9XG4gICAgICB9XG5cbiAgICAgIGlmIChpc0VudW0pIHtcbiAgICAgICAgcGF5bG9hZC5jb250ZW50ID0gdGhpcy5lbnVtU2VyaWFsaXNlQ29udGVudChlbGVtZW50LCBwYXlsb2FkKTtcbiAgICAgIH0gZWxzZSBpZiAodGhpc1tlbGVtZW50LmVsZW1lbnQgKyAnU2VyaWFsaXNlQ29udGVudCddKSB7XG4gICAgICAgIHBheWxvYWQuY29udGVudCA9IHRoaXNbZWxlbWVudC5lbGVtZW50ICsgJ1NlcmlhbGlzZUNvbnRlbnQnXShlbGVtZW50LCBwYXlsb2FkKTtcbiAgICAgIH0gZWxzZSBpZiAoZWxlbWVudC5jb250ZW50ICE9PSB1bmRlZmluZWQpIHtcbiAgICAgICAgdmFyIGNvbnRlbnQgPSB2b2lkIDA7XG5cbiAgICAgICAgaWYgKHZhcmlhYmxlICYmIGVsZW1lbnQuY29udGVudC5rZXkpIHtcbiAgICAgICAgICBjb250ZW50ID0gZWxlbWVudC5jb250ZW50LmNsb25lKCk7XG4gICAgICAgICAgY29udGVudC5rZXkuYXR0cmlidXRlcy5zZXQoJ3ZhcmlhYmxlJywgdmFyaWFibGUpO1xuICAgICAgICAgIGNvbnRlbnQgPSB0aGlzLnNlcmlhbGlzZUNvbnRlbnQoY29udGVudCk7XG4gICAgICAgIH0gZWxzZSB7XG4gICAgICAgICAgY29udGVudCA9IHRoaXMuc2VyaWFsaXNlQ29udGVudChlbGVtZW50LmNvbnRlbnQpO1xuICAgICAgICB9XG5cbiAgICAgICAgaWYgKHRoaXMuc2hvdWxkU2VyaWFsaXNlQ29udGVudChlbGVtZW50LCBjb250ZW50KSkge1xuICAgICAgICAgIHBheWxvYWQuY29udGVudCA9IGNvbnRlbnQ7XG4gICAgICAgIH1cbiAgICAgIH0gZWxzZSBpZiAodGhpcy5zaG91bGRTZXJpYWxpc2VDb250ZW50KGVsZW1lbnQsIGVsZW1lbnQuY29udGVudCkgJiYgZWxlbWVudCBpbnN0YW5jZW9mIHRoaXMubmFtZXNwYWNlLmVsZW1lbnRzLkFycmF5KSB7XG4gICAgICAgIHBheWxvYWQuY29udGVudCA9IFtdO1xuICAgICAgfVxuXG4gICAgICByZXR1cm4gcGF5bG9hZDtcbiAgICB9XG4gIH0sIHtcbiAgICBrZXk6ICdzaG91bGRTZXJpYWxpc2VDb250ZW50JyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gc2hvdWxkU2VyaWFsaXNlQ29udGVudChlbGVtZW50LCBjb250ZW50KSB7XG4gICAgICBpZiAoZWxlbWVudC5lbGVtZW50ID09PSAncGFyc2VSZXN1bHQnIHx8IGVsZW1lbnQuZWxlbWVudCA9PT0gJ2h0dHBSZXF1ZXN0JyB8fCBlbGVtZW50LmVsZW1lbnQgPT09ICdodHRwUmVzcG9uc2UnIHx8IGVsZW1lbnQuZWxlbWVudCA9PT0gJ2NhdGVnb3J5JyB8fCBlbGVtZW50LmVsZW1lbnQgPT09ICdsaW5rJykge1xuICAgICAgICByZXR1cm4gdHJ1ZTtcbiAgICAgIH1cblxuICAgICAgaWYgKGNvbnRlbnQgPT09IHVuZGVmaW5lZCkge1xuICAgICAgICByZXR1cm4gZmFsc2U7XG4gICAgICB9XG5cbiAgICAgIGlmIChBcnJheS5pc0FycmF5KGNvbnRlbnQpICYmIGNvbnRlbnQubGVuZ3RoID09PSAwKSB7XG4gICAgICAgIHJldHVybiBmYWxzZTtcbiAgICAgIH1cblxuICAgICAgcmV0dXJuIHRydWU7XG4gICAgfVxuICB9LCB7XG4gICAga2V5OiAncmVmU2VyaWFsaXNlQ29udGVudCcsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIHJlZlNlcmlhbGlzZUNvbnRlbnQoZWxlbWVudCwgcGF5bG9hZCkge1xuICAgICAgZGVsZXRlIHBheWxvYWQuYXR0cmlidXRlcztcblxuICAgICAgcmV0dXJuIHtcbiAgICAgICAgaHJlZjogZWxlbWVudC50b1ZhbHVlKCksXG4gICAgICAgIHBhdGg6IGVsZW1lbnQucGF0aC50b1ZhbHVlKClcbiAgICAgIH07XG4gICAgfVxuICB9LCB7XG4gICAga2V5OiAnc291cmNlTWFwU2VyaWFsaXNlQ29udGVudCcsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIHNvdXJjZU1hcFNlcmlhbGlzZUNvbnRlbnQoZWxlbWVudCkge1xuICAgICAgcmV0dXJuIGVsZW1lbnQudG9WYWx1ZSgpO1xuICAgIH1cbiAgfSwge1xuICAgIGtleTogJ2RhdGFTdHJ1Y3R1cmVTZXJpYWxpc2VDb250ZW50JyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gZGF0YVN0cnVjdHVyZVNlcmlhbGlzZUNvbnRlbnQoZWxlbWVudCkge1xuICAgICAgcmV0dXJuIFt0aGlzLnNlcmlhbGlzZUNvbnRlbnQoZWxlbWVudC5jb250ZW50KV07XG4gICAgfVxuICB9LCB7XG4gICAga2V5OiAnZW51bVNlcmlhbGlzZUF0dHJpYnV0ZXMnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBlbnVtU2VyaWFsaXNlQXR0cmlidXRlcyhlbGVtZW50KSB7XG4gICAgICB2YXIgX3RoaXMyID0gdGhpcztcblxuICAgICAgdmFyIGF0dHJpYnV0ZXMgPSBlbGVtZW50LmF0dHJpYnV0ZXMuY2xvbmUoKTtcblxuICAgICAgLy8gRW51bWVyYXRpb25zIGF0dHJpYnV0ZSB3YXMgaXMgcGxhY2VkIGluc2lkZSBjb250ZW50IChzZWUgYGVudW1TZXJpYWxpc2VDb250ZW50YCBiZWxvdylcbiAgICAgIHZhciBlbnVtZXJhdGlvbnMgPSBhdHRyaWJ1dGVzLnJlbW92ZSgnZW51bWVyYXRpb25zJykgfHwgbmV3IHRoaXMubmFtZXNwYWNlLmVsZW1lbnRzLkFycmF5KFtdKTtcblxuICAgICAgLy8gUmVtb3ZlIGZpeGVkIHR5cGUgYXR0cmlidXRlIGZyb20gc2FtcGxlcyBhbmQgZGVmYXVsdFxuICAgICAgdmFyIGRlZmF1bHRWYWx1ZSA9IGF0dHJpYnV0ZXMuZ2V0KCdkZWZhdWx0Jyk7XG4gICAgICB2YXIgc2FtcGxlcyA9IGF0dHJpYnV0ZXMuZ2V0KCdzYW1wbGVzJykgfHwgbmV3IHRoaXMubmFtZXNwYWNlLmVsZW1lbnRzLkFycmF5KFtdKTtcblxuICAgICAgaWYgKGRlZmF1bHRWYWx1ZSAmJiBkZWZhdWx0VmFsdWUuY29udGVudCkge1xuICAgICAgICBpZiAoZGVmYXVsdFZhbHVlLmNvbnRlbnQuYXR0cmlidXRlcykge1xuICAgICAgICAgIGRlZmF1bHRWYWx1ZS5jb250ZW50LmF0dHJpYnV0ZXMucmVtb3ZlKCd0eXBlQXR0cmlidXRlcycpO1xuICAgICAgICB9XG4gICAgICAgIC8vIFdyYXAgZGVmYXVsdCBpbiBhcnJheSAobm90IHN1cmUgaXQgaXMgcmVhbGx5IG5lZWRlZCBiZWNhdXNlIHRlc3RzIHBhc3Mgd2l0aG91dCB0aGlzIGxpbmUpXG4gICAgICAgIGF0dHJpYnV0ZXMuc2V0KCdkZWZhdWx0JywgbmV3IHRoaXMubmFtZXNwYWNlLmVsZW1lbnRzLkFycmF5KFtkZWZhdWx0VmFsdWUuY29udGVudF0pKTtcbiAgICAgIH1cblxuICAgICAgLy8gU3RyaXAgdHlwZUF0dHJpYnV0ZXMgZnJvbSBzYW1wbGVzLCAwLjYgZG9lc24ndCB1c3VhbGx5IGNvbnRhaW4gdGhlbSBpbiBzYW1wbGVzXG4gICAgICBzYW1wbGVzLmZvckVhY2goZnVuY3Rpb24gKHNhbXBsZSkge1xuICAgICAgICBpZiAoc2FtcGxlLmNvbnRlbnQgJiYgc2FtcGxlLmNvbnRlbnQuZWxlbWVudCkge1xuICAgICAgICAgIHNhbXBsZS5jb250ZW50LmF0dHJpYnV0ZXMucmVtb3ZlKCd0eXBlQXR0cmlidXRlcycpO1xuICAgICAgICB9XG4gICAgICB9KTtcblxuICAgICAgLy8gQ29udGVudCAtPiBTYW1wbGVzXG4gICAgICBpZiAoZWxlbWVudC5jb250ZW50ICYmIGVudW1lcmF0aW9ucy5sZW5ndGggIT09IDApIHtcbiAgICAgICAgLy8gSWYgd2UgZG9uJ3QgaGF2ZSBlbnVtZXJhdGlvbnMsIGNvbnRlbnQgc2hvdWxkIHN0YXkgaW5cbiAgICAgICAgLy8gY29udGVudCAoZW51bWVyYXRpb25zKSBhcyBwZXIgRHJhZnRlciAzIGJlaGF2aW91ci5cbiAgICAgICAgc2FtcGxlcy51bnNoaWZ0KGVsZW1lbnQuY29udGVudCk7XG4gICAgICB9XG5cbiAgICAgIHNhbXBsZXMgPSBzYW1wbGVzLm1hcChmdW5jdGlvbiAoc2FtcGxlKSB7XG4gICAgICAgIGlmIChzYW1wbGUgaW5zdGFuY2VvZiBfdGhpczIubmFtZXNwYWNlLmVsZW1lbnRzLkFycmF5KSB7XG4gICAgICAgICAgcmV0dXJuIFtzYW1wbGVdO1xuICAgICAgICB9XG5cbiAgICAgICAgcmV0dXJuIG5ldyBfdGhpczIubmFtZXNwYWNlLmVsZW1lbnRzLkFycmF5KFtzYW1wbGUuY29udGVudF0pO1xuICAgICAgfSk7XG5cbiAgICAgIGlmIChzYW1wbGVzLmxlbmd0aCkge1xuICAgICAgICBhdHRyaWJ1dGVzLnNldCgnc2FtcGxlcycsIHNhbXBsZXMpO1xuICAgICAgfVxuXG4gICAgICBpZiAoYXR0cmlidXRlcy5sZW5ndGggPiAwKSB7XG4gICAgICAgIHJldHVybiB0aGlzLnNlcmlhbGlzZU9iamVjdChhdHRyaWJ1dGVzKTtcbiAgICAgIH1cblxuICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICB9XG4gIH0sIHtcbiAgICBrZXk6ICdlbnVtU2VyaWFsaXNlQ29udGVudCcsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIGVudW1TZXJpYWxpc2VDb250ZW50KGVsZW1lbnQpIHtcbiAgICAgIHZhciBfdGhpczMgPSB0aGlzO1xuXG4gICAgICAvLyBJbiBBUEkgRWxlbWVudHMgPCAxLjAsIHRoZSBjb250ZW50IGlzIHRoZSBlbnVtZXJhdGlvbnNcbiAgICAgIC8vIElmIHdlIGRvbid0IGhhdmUgYW4gZW51bWVyYXRpb25zLCB1c2UgdGhlIHZhbHVlIChEcmFmdGVyIDMgYmVoYXZpb3VyKVxuXG4gICAgICBpZiAoZWxlbWVudC5fYXR0cmlidXRlcykge1xuICAgICAgICB2YXIgZW51bWVyYXRpb25zID0gZWxlbWVudC5hdHRyaWJ1dGVzLmdldCgnZW51bWVyYXRpb25zJyk7XG5cbiAgICAgICAgaWYgKGVudW1lcmF0aW9ucyAmJiBlbnVtZXJhdGlvbnMubGVuZ3RoID4gMCkge1xuICAgICAgICAgIHJldHVybiBlbnVtZXJhdGlvbnMuY29udGVudC5tYXAoZnVuY3Rpb24gKGVudW1lcmF0aW9uKSB7XG4gICAgICAgICAgICB2YXIgZSA9IGVudW1lcmF0aW9uLmNsb25lKCk7XG4gICAgICAgICAgICBlLmF0dHJpYnV0ZXMucmVtb3ZlKCd0eXBlQXR0cmlidXRlcycpO1xuICAgICAgICAgICAgcmV0dXJuIF90aGlzMy5zZXJpYWxpc2UoZSk7XG4gICAgICAgICAgfSk7XG4gICAgICAgIH1cbiAgICAgIH1cblxuICAgICAgaWYgKGVsZW1lbnQuY29udGVudCkge1xuICAgICAgICB2YXIgdmFsdWUgPSBlbGVtZW50LmNvbnRlbnQuY2xvbmUoKTtcbiAgICAgICAgdmFsdWUuYXR0cmlidXRlcy5yZW1vdmUoJ3R5cGVBdHRyaWJ1dGVzJyk7XG4gICAgICAgIHJldHVybiBbdGhpcy5zZXJpYWxpc2UodmFsdWUpXTtcbiAgICAgIH1cblxuICAgICAgcmV0dXJuIFtdO1xuICAgIH1cbiAgfSwge1xuICAgIGtleTogJ2Rlc2VyaWFsaXNlJyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gZGVzZXJpYWxpc2UodmFsdWUpIHtcbiAgICAgIGlmICh0eXBlb2YgdmFsdWUgPT09ICdzdHJpbmcnKSB7XG4gICAgICAgIHJldHVybiBuZXcgdGhpcy5uYW1lc3BhY2UuZWxlbWVudHMuU3RyaW5nKHZhbHVlKTtcbiAgICAgIH1cblxuICAgICAgaWYgKHR5cGVvZiB2YWx1ZSA9PT0gJ251bWJlcicpIHtcbiAgICAgICAgcmV0dXJuIG5ldyB0aGlzLm5hbWVzcGFjZS5lbGVtZW50cy5OdW1iZXIodmFsdWUpO1xuICAgICAgfVxuXG4gICAgICBpZiAodHlwZW9mIHZhbHVlID09PSAnYm9vbGVhbicpIHtcbiAgICAgICAgcmV0dXJuIG5ldyB0aGlzLm5hbWVzcGFjZS5lbGVtZW50cy5Cb29sZWFuKHZhbHVlKTtcbiAgICAgIH1cblxuICAgICAgaWYgKHZhbHVlID09PSBudWxsKSB7XG4gICAgICAgIHJldHVybiBuZXcgdGhpcy5uYW1lc3BhY2UuZWxlbWVudHMuTnVsbCgpO1xuICAgICAgfVxuXG4gICAgICBpZiAoQXJyYXkuaXNBcnJheSh2YWx1ZSkpIHtcbiAgICAgICAgcmV0dXJuIG5ldyB0aGlzLm5hbWVzcGFjZS5lbGVtZW50cy5BcnJheSh2YWx1ZS5tYXAodGhpcy5kZXNlcmlhbGlzZSwgdGhpcykpO1xuICAgICAgfVxuXG4gICAgICB2YXIgRWxlbWVudENsYXNzID0gdGhpcy5uYW1lc3BhY2UuZ2V0RWxlbWVudENsYXNzKHZhbHVlLmVsZW1lbnQpO1xuICAgICAgdmFyIGVsZW1lbnQgPSBuZXcgRWxlbWVudENsYXNzKCk7XG5cbiAgICAgIGlmIChlbGVtZW50LmVsZW1lbnQgIT09IHZhbHVlLmVsZW1lbnQpIHtcbiAgICAgICAgZWxlbWVudC5lbGVtZW50ID0gdmFsdWUuZWxlbWVudDtcbiAgICAgIH1cblxuICAgICAgaWYgKHZhbHVlLm1ldGEpIHtcbiAgICAgICAgdGhpcy5kZXNlcmlhbGlzZU9iamVjdCh2YWx1ZS5tZXRhLCBlbGVtZW50Lm1ldGEpO1xuICAgICAgfVxuXG4gICAgICBpZiAodmFsdWUuYXR0cmlidXRlcykge1xuICAgICAgICB0aGlzLmRlc2VyaWFsaXNlT2JqZWN0KHZhbHVlLmF0dHJpYnV0ZXMsIGVsZW1lbnQuYXR0cmlidXRlcyk7XG4gICAgICB9XG5cbiAgICAgIHZhciBjb250ZW50ID0gdGhpcy5kZXNlcmlhbGlzZUNvbnRlbnQodmFsdWUuY29udGVudCk7XG4gICAgICBpZiAoY29udGVudCAhPT0gdW5kZWZpbmVkIHx8IGVsZW1lbnQuY29udGVudCA9PT0gbnVsbCkge1xuICAgICAgICBlbGVtZW50LmNvbnRlbnQgPSBjb250ZW50O1xuICAgICAgfVxuXG4gICAgICBpZiAoZWxlbWVudC5lbGVtZW50ID09PSAnZW51bScpIHtcbiAgICAgICAgLy8gR3JhYiBlbnVtZXJhdGlvbnMgZnJvbSBjb250ZW50XG4gICAgICAgIGlmIChlbGVtZW50LmNvbnRlbnQpIHtcbiAgICAgICAgICBlbGVtZW50LmF0dHJpYnV0ZXMuc2V0KCdlbnVtZXJhdGlvbnMnLCBlbGVtZW50LmNvbnRlbnQpO1xuICAgICAgICB9XG5cbiAgICAgICAgLy8gVW53cmFwIHRoZSBzYW1wbGUgdmFsdWUgKGluc2lkZSBkb3VibGUgYXJyYXkpXG4gICAgICAgIHZhciBzYW1wbGVzID0gZWxlbWVudC5hdHRyaWJ1dGVzLmdldCgnc2FtcGxlcycpO1xuICAgICAgICBlbGVtZW50LmF0dHJpYnV0ZXMucmVtb3ZlKCdzYW1wbGVzJyk7XG5cbiAgICAgICAgaWYgKHNhbXBsZXMpIHtcbiAgICAgICAgICAvLyBSZS13cmFwIHNhbXBsZXMgZnJvbSBhcnJheSBvZiBhcnJheSB0byBhcnJheSBvZiBlbnVtJ3NcblxuICAgICAgICAgIHZhciBleGlzdGluZ1NhbXBsZXMgPSBzYW1wbGVzO1xuXG4gICAgICAgICAgc2FtcGxlcyA9IG5ldyB0aGlzLm5hbWVzcGFjZS5lbGVtZW50cy5BcnJheSgpO1xuICAgICAgICAgIGV4aXN0aW5nU2FtcGxlcy5mb3JFYWNoKGZ1bmN0aW9uIChleGlzdGluZ1NhbXBsZSkge1xuICAgICAgICAgICAgZXhpc3RpbmdTYW1wbGUuZm9yRWFjaChmdW5jdGlvbiAoc2FtcGxlKSB7XG4gICAgICAgICAgICAgIHZhciBlbnVtRWxlbWVudCA9IG5ldyBFbGVtZW50Q2xhc3Moc2FtcGxlKTtcbiAgICAgICAgICAgICAgZW51bUVsZW1lbnQuZWxlbWVudCA9IGVsZW1lbnQuZWxlbWVudDtcbiAgICAgICAgICAgICAgc2FtcGxlcy5wdXNoKGVudW1FbGVtZW50KTtcbiAgICAgICAgICAgIH0pO1xuICAgICAgICAgIH0pO1xuXG4gICAgICAgICAgdmFyIHNhbXBsZSA9IHNhbXBsZXMuc2hpZnQoKTtcblxuICAgICAgICAgIGlmIChzYW1wbGUpIHtcbiAgICAgICAgICAgIGVsZW1lbnQuY29udGVudCA9IHNhbXBsZS5jb250ZW50O1xuICAgICAgICAgIH0gZWxzZSB7XG4gICAgICAgICAgICBlbGVtZW50LmNvbnRlbnQgPSB1bmRlZmluZWQ7XG4gICAgICAgICAgfVxuXG4gICAgICAgICAgZWxlbWVudC5hdHRyaWJ1dGVzLnNldCgnc2FtcGxlcycsIHNhbXBsZXMpO1xuICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgIGVsZW1lbnQuY29udGVudCA9IHVuZGVmaW5lZDtcbiAgICAgICAgfVxuXG4gICAgICAgIC8vIFVud3JhcCB0aGUgZGVmYXVsdCB2YWx1ZVxuICAgICAgICB2YXIgZGVmYXVsdFZhbHVlID0gZWxlbWVudC5hdHRyaWJ1dGVzLmdldCgnZGVmYXVsdCcpO1xuICAgICAgICBpZiAoZGVmYXVsdFZhbHVlICYmIGRlZmF1bHRWYWx1ZS5sZW5ndGggPiAwKSB7XG4gICAgICAgICAgZGVmYXVsdFZhbHVlID0gZGVmYXVsdFZhbHVlLmdldCgwKTtcbiAgICAgICAgICB2YXIgZGVmYXVsdEVsZW1lbnQgPSBuZXcgRWxlbWVudENsYXNzKGRlZmF1bHRWYWx1ZSk7XG4gICAgICAgICAgZGVmYXVsdEVsZW1lbnQuZWxlbWVudCA9IGVsZW1lbnQuZWxlbWVudDtcbiAgICAgICAgICBlbGVtZW50LmF0dHJpYnV0ZXMuc2V0KCdkZWZhdWx0JywgZGVmYXVsdEVsZW1lbnQpO1xuICAgICAgICB9XG4gICAgICB9IGVsc2UgaWYgKGVsZW1lbnQuZWxlbWVudCA9PT0gJ2RhdGFTdHJ1Y3R1cmUnICYmIEFycmF5LmlzQXJyYXkoZWxlbWVudC5jb250ZW50KSkge1xuICAgICAgICB2YXIgX2VsZW1lbnQkY29udGVudCA9IF9zbGljZWRUb0FycmF5KGVsZW1lbnQuY29udGVudCwgMSk7XG5cbiAgICAgICAgZWxlbWVudC5jb250ZW50ID0gX2VsZW1lbnQkY29udGVudFswXTtcbiAgICAgIH0gZWxzZSBpZiAoZWxlbWVudC5lbGVtZW50ID09PSAnY2F0ZWdvcnknKSB7XG4gICAgICAgIC8vIFwibWV0YVwiIGF0dHJpYnV0ZSBoYXMgYmVlbiByZW5hbWVkIHRvIG1ldGFkYXRhXG4gICAgICAgIHZhciBtZXRhZGF0YSA9IGVsZW1lbnQuYXR0cmlidXRlcy5nZXQoJ21ldGEnKTtcblxuICAgICAgICBpZiAobWV0YWRhdGEpIHtcbiAgICAgICAgICBlbGVtZW50LmF0dHJpYnV0ZXMuc2V0KCdtZXRhZGF0YScsIG1ldGFkYXRhKTtcbiAgICAgICAgICBlbGVtZW50LmF0dHJpYnV0ZXMucmVtb3ZlKCdtZXRhJyk7XG4gICAgICAgIH1cbiAgICAgIH0gZWxzZSBpZiAoZWxlbWVudC5lbGVtZW50ID09PSAnbWVtYmVyJyAmJiBlbGVtZW50LmtleSAmJiBlbGVtZW50LmtleS5fYXR0cmlidXRlcyAmJiBlbGVtZW50LmtleS5fYXR0cmlidXRlcy5nZXRWYWx1ZSgndmFyaWFibGUnKSkge1xuICAgICAgICBlbGVtZW50LmF0dHJpYnV0ZXMuc2V0KCd2YXJpYWJsZScsIGVsZW1lbnQua2V5LmF0dHJpYnV0ZXMuZ2V0KCd2YXJpYWJsZScpKTtcbiAgICAgICAgZWxlbWVudC5rZXkuYXR0cmlidXRlcy5yZW1vdmUoJ3ZhcmlhYmxlJyk7XG4gICAgICB9XG5cbiAgICAgIHJldHVybiBlbGVtZW50O1xuICAgIH1cblxuICAgIC8vIFByaXZhdGUgQVBJXG5cbiAgfSwge1xuICAgIGtleTogJ3NlcmlhbGlzZUNvbnRlbnQnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBzZXJpYWxpc2VDb250ZW50KGNvbnRlbnQpIHtcbiAgICAgIGlmIChjb250ZW50IGluc3RhbmNlb2YgdGhpcy5uYW1lc3BhY2UuZWxlbWVudHMuRWxlbWVudCkge1xuICAgICAgICByZXR1cm4gdGhpcy5zZXJpYWxpc2UoY29udGVudCk7XG4gICAgICB9XG5cbiAgICAgIGlmIChjb250ZW50IGluc3RhbmNlb2YgdGhpcy5uYW1lc3BhY2UuS2V5VmFsdWVQYWlyKSB7XG4gICAgICAgIHZhciBwYWlyID0ge1xuICAgICAgICAgIGtleTogdGhpcy5zZXJpYWxpc2UoY29udGVudC5rZXkpXG4gICAgICAgIH07XG5cbiAgICAgICAgaWYgKGNvbnRlbnQudmFsdWUpIHtcbiAgICAgICAgICBwYWlyLnZhbHVlID0gdGhpcy5zZXJpYWxpc2UoY29udGVudC52YWx1ZSk7XG4gICAgICAgIH1cblxuICAgICAgICByZXR1cm4gcGFpcjtcbiAgICAgIH1cblxuICAgICAgaWYgKGNvbnRlbnQgJiYgY29udGVudC5tYXApIHtcbiAgICAgICAgcmV0dXJuIGNvbnRlbnQubWFwKHRoaXMuc2VyaWFsaXNlLCB0aGlzKTtcbiAgICAgIH1cblxuICAgICAgcmV0dXJuIGNvbnRlbnQ7XG4gICAgfVxuICB9LCB7XG4gICAga2V5OiAnZGVzZXJpYWxpc2VDb250ZW50JyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gZGVzZXJpYWxpc2VDb250ZW50KGNvbnRlbnQpIHtcbiAgICAgIGlmIChjb250ZW50KSB7XG4gICAgICAgIGlmIChjb250ZW50LmVsZW1lbnQpIHtcbiAgICAgICAgICByZXR1cm4gdGhpcy5kZXNlcmlhbGlzZShjb250ZW50KTtcbiAgICAgICAgfVxuXG4gICAgICAgIGlmIChjb250ZW50LmtleSkge1xuICAgICAgICAgIHZhciBwYWlyID0gbmV3IHRoaXMubmFtZXNwYWNlLktleVZhbHVlUGFpcih0aGlzLmRlc2VyaWFsaXNlKGNvbnRlbnQua2V5KSk7XG5cbiAgICAgICAgICBpZiAoY29udGVudC52YWx1ZSkge1xuICAgICAgICAgICAgcGFpci52YWx1ZSA9IHRoaXMuZGVzZXJpYWxpc2UoY29udGVudC52YWx1ZSk7XG4gICAgICAgICAgfVxuXG4gICAgICAgICAgcmV0dXJuIHBhaXI7XG4gICAgICAgIH1cblxuICAgICAgICBpZiAoY29udGVudC5tYXApIHtcbiAgICAgICAgICByZXR1cm4gY29udGVudC5tYXAodGhpcy5kZXNlcmlhbGlzZSwgdGhpcyk7XG4gICAgICAgIH1cbiAgICAgIH1cblxuICAgICAgcmV0dXJuIGNvbnRlbnQ7XG4gICAgfVxuICB9LCB7XG4gICAga2V5OiAnc2hvdWxkUmVmcmFjdCcsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIHNob3VsZFJlZnJhY3QoZWxlbWVudCkge1xuICAgICAgaWYgKGVsZW1lbnQuX2F0dHJpYnV0ZXMgJiYgZWxlbWVudC5hdHRyaWJ1dGVzLmtleXMoKS5sZW5ndGggfHwgZWxlbWVudC5fbWV0YSAmJiBlbGVtZW50Lm1ldGEua2V5cygpLmxlbmd0aCkge1xuICAgICAgICByZXR1cm4gdHJ1ZTtcbiAgICAgIH1cblxuICAgICAgaWYgKGVsZW1lbnQuZWxlbWVudCA9PT0gJ2VudW0nKSB7XG4gICAgICAgIC8vIGVudW0gZWxlbWVudHMgYXJlIHRyZWF0ZWQgbGlrZSBwcmltaXRpdmVzIChhcnJheSlcbiAgICAgICAgcmV0dXJuIGZhbHNlO1xuICAgICAgfVxuXG4gICAgICBpZiAoZWxlbWVudC5lbGVtZW50ICE9PSBlbGVtZW50LnByaW1pdGl2ZSgpIHx8IGVsZW1lbnQuZWxlbWVudCA9PT0gJ21lbWJlcicpIHtcbiAgICAgICAgcmV0dXJuIHRydWU7XG4gICAgICB9XG5cbiAgICAgIHJldHVybiBmYWxzZTtcbiAgICB9XG4gIH0sIHtcbiAgICBrZXk6ICdjb252ZXJ0S2V5VG9SZWZyYWN0JyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gY29udmVydEtleVRvUmVmcmFjdChrZXksIGl0ZW0pIHtcbiAgICAgIHZhciBfdGhpczQgPSB0aGlzO1xuXG4gICAgICBpZiAodGhpcy5zaG91bGRSZWZyYWN0KGl0ZW0pKSB7XG4gICAgICAgIHJldHVybiB0aGlzLnNlcmlhbGlzZShpdGVtKTtcbiAgICAgIH1cblxuICAgICAgaWYgKGl0ZW0uZWxlbWVudCA9PT0gJ2VudW0nKSB7XG4gICAgICAgIHJldHVybiB0aGlzLnNlcmlhbGlzZUVudW0oaXRlbSk7XG4gICAgICB9XG5cbiAgICAgIGlmIChpdGVtLmVsZW1lbnQgPT09ICdhcnJheScpIHtcbiAgICAgICAgcmV0dXJuIGl0ZW0ubWFwKGZ1bmN0aW9uIChzdWJJdGVtKSB7XG4gICAgICAgICAgaWYgKF90aGlzNC5zaG91bGRSZWZyYWN0KHN1Ykl0ZW0pIHx8IGtleSA9PT0gJ2RlZmF1bHQnKSB7XG4gICAgICAgICAgICByZXR1cm4gX3RoaXM0LnNlcmlhbGlzZShzdWJJdGVtKTtcbiAgICAgICAgICB9XG5cbiAgICAgICAgICBpZiAoc3ViSXRlbS5lbGVtZW50ID09PSAnYXJyYXknIHx8IHN1Ykl0ZW0uZWxlbWVudCA9PT0gJ29iamVjdCcgfHwgc3ViSXRlbS5lbGVtZW50ID09PSAnZW51bScpIHtcbiAgICAgICAgICAgIC8vIGl0ZW1zIGZvciBhcnJheSBvciBlbnVtIGluc2lkZSBhcnJheSBhcmUgYWx3YXlzIHNlcmlhbGlzZWRcbiAgICAgICAgICAgIHJldHVybiBzdWJJdGVtLmNoaWxkcmVuLm1hcChmdW5jdGlvbiAoc3ViU3ViSXRlbSkge1xuICAgICAgICAgICAgICByZXR1cm4gX3RoaXM0LnNlcmlhbGlzZShzdWJTdWJJdGVtKTtcbiAgICAgICAgICAgIH0pO1xuICAgICAgICAgIH1cblxuICAgICAgICAgIHJldHVybiBzdWJJdGVtLnRvVmFsdWUoKTtcbiAgICAgICAgfSk7XG4gICAgICB9XG5cbiAgICAgIGlmIChpdGVtLmVsZW1lbnQgPT09ICdvYmplY3QnKSB7XG4gICAgICAgIHJldHVybiAoaXRlbS5jb250ZW50IHx8IFtdKS5tYXAodGhpcy5zZXJpYWxpc2UsIHRoaXMpO1xuICAgICAgfVxuXG4gICAgICByZXR1cm4gaXRlbS50b1ZhbHVlKCk7XG4gICAgfVxuICB9LCB7XG4gICAga2V5OiAnc2VyaWFsaXNlRW51bScsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIHNlcmlhbGlzZUVudW0oZWxlbWVudCkge1xuICAgICAgdmFyIF90aGlzNSA9IHRoaXM7XG5cbiAgICAgIHJldHVybiBlbGVtZW50LmNoaWxkcmVuLm1hcChmdW5jdGlvbiAoaXRlbSkge1xuICAgICAgICByZXR1cm4gX3RoaXM1LnNlcmlhbGlzZShpdGVtKTtcbiAgICAgIH0pO1xuICAgIH1cbiAgfSwge1xuICAgIGtleTogJ3NlcmlhbGlzZU9iamVjdCcsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIHNlcmlhbGlzZU9iamVjdChvYmopIHtcbiAgICAgIHZhciBfdGhpczYgPSB0aGlzO1xuXG4gICAgICB2YXIgcmVzdWx0ID0ge307XG5cbiAgICAgIG9iai5mb3JFYWNoKGZ1bmN0aW9uICh2YWx1ZSwga2V5KSB7XG4gICAgICAgIGlmICh2YWx1ZSkge1xuICAgICAgICAgIHZhciBrZXlWYWx1ZSA9IGtleS50b1ZhbHVlKCk7XG4gICAgICAgICAgcmVzdWx0W2tleVZhbHVlXSA9IF90aGlzNi5jb252ZXJ0S2V5VG9SZWZyYWN0KGtleVZhbHVlLCB2YWx1ZSk7XG4gICAgICAgIH1cbiAgICAgIH0pO1xuXG4gICAgICByZXR1cm4gcmVzdWx0O1xuICAgIH1cbiAgfSwge1xuICAgIGtleTogJ2Rlc2VyaWFsaXNlT2JqZWN0JyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gZGVzZXJpYWxpc2VPYmplY3QoZnJvbSwgdG8pIHtcbiAgICAgIHZhciBfdGhpczcgPSB0aGlzO1xuXG4gICAgICBPYmplY3Qua2V5cyhmcm9tKS5mb3JFYWNoKGZ1bmN0aW9uIChrZXkpIHtcbiAgICAgICAgdG8uc2V0KGtleSwgX3RoaXM3LmRlc2VyaWFsaXNlKGZyb21ba2V5XSkpO1xuICAgICAgfSk7XG4gICAgfVxuICB9XSk7XG5cbiAgcmV0dXJuIEpTT04wNlNlcmlhbGlzZXI7XG59KEpTT05TZXJpYWxpc2VyKTsiLCIndXNlIHN0cmljdCc7XG5cbnZhciBfY3JlYXRlQ2xhc3MgPSBmdW5jdGlvbiAoKSB7IGZ1bmN0aW9uIGRlZmluZVByb3BlcnRpZXModGFyZ2V0LCBwcm9wcykgeyBmb3IgKHZhciBpID0gMDsgaSA8IHByb3BzLmxlbmd0aDsgaSsrKSB7IHZhciBkZXNjcmlwdG9yID0gcHJvcHNbaV07IGRlc2NyaXB0b3IuZW51bWVyYWJsZSA9IGRlc2NyaXB0b3IuZW51bWVyYWJsZSB8fCBmYWxzZTsgZGVzY3JpcHRvci5jb25maWd1cmFibGUgPSB0cnVlOyBpZiAoXCJ2YWx1ZVwiIGluIGRlc2NyaXB0b3IpIGRlc2NyaXB0b3Iud3JpdGFibGUgPSB0cnVlOyBPYmplY3QuZGVmaW5lUHJvcGVydHkodGFyZ2V0LCBkZXNjcmlwdG9yLmtleSwgZGVzY3JpcHRvcik7IH0gfSByZXR1cm4gZnVuY3Rpb24gKENvbnN0cnVjdG9yLCBwcm90b1Byb3BzLCBzdGF0aWNQcm9wcykgeyBpZiAocHJvdG9Qcm9wcykgZGVmaW5lUHJvcGVydGllcyhDb25zdHJ1Y3Rvci5wcm90b3R5cGUsIHByb3RvUHJvcHMpOyBpZiAoc3RhdGljUHJvcHMpIGRlZmluZVByb3BlcnRpZXMoQ29uc3RydWN0b3IsIHN0YXRpY1Byb3BzKTsgcmV0dXJuIENvbnN0cnVjdG9yOyB9OyB9KCk7XG5cbmZ1bmN0aW9uIF9jbGFzc0NhbGxDaGVjayhpbnN0YW5jZSwgQ29uc3RydWN0b3IpIHsgaWYgKCEoaW5zdGFuY2UgaW5zdGFuY2VvZiBDb25zdHJ1Y3RvcikpIHsgdGhyb3cgbmV3IFR5cGVFcnJvcihcIkNhbm5vdCBjYWxsIGEgY2xhc3MgYXMgYSBmdW5jdGlvblwiKTsgfSB9XG5cbi8qKlxuICogQGNsYXNzIEpTT05TZXJpYWxpc2VyXG4gKlxuICogQHBhcmFtIHtOYW1lc3BhY2V9IG5hbWVzcGFjZVxuICpcbiAqIEBwcm9wZXJ0eSB7TmFtZXNwYWNlfSBuYW1lc3BhY2VcbiAqL1xudmFyIEpTT05TZXJpYWxpc2VyID0gZnVuY3Rpb24gKCkge1xuICBmdW5jdGlvbiBKU09OU2VyaWFsaXNlcihuYW1lc3BhY2UpIHtcbiAgICBfY2xhc3NDYWxsQ2hlY2sodGhpcywgSlNPTlNlcmlhbGlzZXIpO1xuXG4gICAgdGhpcy5uYW1lc3BhY2UgPSBuYW1lc3BhY2UgfHwgbmV3IHRoaXMuTmFtZXNwYWNlKCk7XG4gIH1cblxuICAvKipcbiAgICogQHBhcmFtIHtFbGVtZW50fSBlbGVtZW50XG4gICAqIEByZXR1cm5zIHtvYmplY3R9XG4gICAqL1xuXG5cbiAgX2NyZWF0ZUNsYXNzKEpTT05TZXJpYWxpc2VyLCBbe1xuICAgIGtleTogJ3NlcmlhbGlzZScsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIHNlcmlhbGlzZShlbGVtZW50KSB7XG4gICAgICBpZiAoIShlbGVtZW50IGluc3RhbmNlb2YgdGhpcy5uYW1lc3BhY2UuZWxlbWVudHMuRWxlbWVudCkpIHtcbiAgICAgICAgdGhyb3cgbmV3IFR5cGVFcnJvcignR2l2ZW4gZWxlbWVudCBgJyArIGVsZW1lbnQgKyAnYCBpcyBub3QgYW4gRWxlbWVudCBpbnN0YW5jZScpO1xuICAgICAgfVxuXG4gICAgICB2YXIgcGF5bG9hZCA9IHtcbiAgICAgICAgZWxlbWVudDogZWxlbWVudC5lbGVtZW50XG4gICAgICB9O1xuXG4gICAgICBpZiAoZWxlbWVudC5fbWV0YSAmJiBlbGVtZW50Ll9tZXRhLmxlbmd0aCA+IDApIHtcbiAgICAgICAgcGF5bG9hZC5tZXRhID0gdGhpcy5zZXJpYWxpc2VPYmplY3QoZWxlbWVudC5tZXRhKTtcbiAgICAgIH1cblxuICAgICAgaWYgKGVsZW1lbnQuX2F0dHJpYnV0ZXMgJiYgZWxlbWVudC5fYXR0cmlidXRlcy5sZW5ndGggPiAwKSB7XG4gICAgICAgIHBheWxvYWQuYXR0cmlidXRlcyA9IHRoaXMuc2VyaWFsaXNlT2JqZWN0KGVsZW1lbnQuYXR0cmlidXRlcyk7XG4gICAgICB9XG5cbiAgICAgIHZhciBjb250ZW50ID0gdGhpcy5zZXJpYWxpc2VDb250ZW50KGVsZW1lbnQuY29udGVudCk7XG5cbiAgICAgIGlmIChjb250ZW50ICE9PSB1bmRlZmluZWQpIHtcbiAgICAgICAgcGF5bG9hZC5jb250ZW50ID0gY29udGVudDtcbiAgICAgIH1cblxuICAgICAgcmV0dXJuIHBheWxvYWQ7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQHBhcmFtIHtvYmplY3R9IHZhbHVlXG4gICAgICogQHJldHVybnMge0VsZW1lbnR9XG4gICAgICovXG5cbiAgfSwge1xuICAgIGtleTogJ2Rlc2VyaWFsaXNlJyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gZGVzZXJpYWxpc2UodmFsdWUpIHtcbiAgICAgIGlmICghdmFsdWUuZWxlbWVudCkge1xuICAgICAgICB0aHJvdyBuZXcgRXJyb3IoJ0dpdmVuIHZhbHVlIGlzIG5vdCBhbiBvYmplY3QgY29udGFpbmluZyBhbiBlbGVtZW50IG5hbWUnKTtcbiAgICAgIH1cblxuICAgICAgdmFyIEVsZW1lbnRDbGFzcyA9IHRoaXMubmFtZXNwYWNlLmdldEVsZW1lbnRDbGFzcyh2YWx1ZS5lbGVtZW50KTtcbiAgICAgIHZhciBlbGVtZW50ID0gbmV3IEVsZW1lbnRDbGFzcygpO1xuXG4gICAgICBpZiAoZWxlbWVudC5lbGVtZW50ICE9PSB2YWx1ZS5lbGVtZW50KSB7XG4gICAgICAgIGVsZW1lbnQuZWxlbWVudCA9IHZhbHVlLmVsZW1lbnQ7XG4gICAgICB9XG5cbiAgICAgIGlmICh2YWx1ZS5tZXRhKSB7XG4gICAgICAgIHRoaXMuZGVzZXJpYWxpc2VPYmplY3QodmFsdWUubWV0YSwgZWxlbWVudC5tZXRhKTtcbiAgICAgIH1cblxuICAgICAgaWYgKHZhbHVlLmF0dHJpYnV0ZXMpIHtcbiAgICAgICAgdGhpcy5kZXNlcmlhbGlzZU9iamVjdCh2YWx1ZS5hdHRyaWJ1dGVzLCBlbGVtZW50LmF0dHJpYnV0ZXMpO1xuICAgICAgfVxuXG4gICAgICB2YXIgY29udGVudCA9IHRoaXMuZGVzZXJpYWxpc2VDb250ZW50KHZhbHVlLmNvbnRlbnQpO1xuICAgICAgaWYgKGNvbnRlbnQgIT09IHVuZGVmaW5lZCB8fCBlbGVtZW50LmNvbnRlbnQgPT09IG51bGwpIHtcbiAgICAgICAgZWxlbWVudC5jb250ZW50ID0gY29udGVudDtcbiAgICAgIH1cblxuICAgICAgcmV0dXJuIGVsZW1lbnQ7XG4gICAgfVxuXG4gICAgLy8gUHJpdmF0ZSBBUElcblxuICB9LCB7XG4gICAga2V5OiAnc2VyaWFsaXNlQ29udGVudCcsXG4gICAgdmFsdWU6IGZ1bmN0aW9uIHNlcmlhbGlzZUNvbnRlbnQoY29udGVudCkge1xuICAgICAgaWYgKGNvbnRlbnQgaW5zdGFuY2VvZiB0aGlzLm5hbWVzcGFjZS5lbGVtZW50cy5FbGVtZW50KSB7XG4gICAgICAgIHJldHVybiB0aGlzLnNlcmlhbGlzZShjb250ZW50KTtcbiAgICAgIH1cblxuICAgICAgaWYgKGNvbnRlbnQgaW5zdGFuY2VvZiB0aGlzLm5hbWVzcGFjZS5LZXlWYWx1ZVBhaXIpIHtcbiAgICAgICAgdmFyIHBhaXIgPSB7XG4gICAgICAgICAga2V5OiB0aGlzLnNlcmlhbGlzZShjb250ZW50LmtleSlcbiAgICAgICAgfTtcblxuICAgICAgICBpZiAoY29udGVudC52YWx1ZSkge1xuICAgICAgICAgIHBhaXIudmFsdWUgPSB0aGlzLnNlcmlhbGlzZShjb250ZW50LnZhbHVlKTtcbiAgICAgICAgfVxuXG4gICAgICAgIHJldHVybiBwYWlyO1xuICAgICAgfVxuXG4gICAgICBpZiAoY29udGVudCAmJiBjb250ZW50Lm1hcCkge1xuICAgICAgICBpZiAoY29udGVudC5sZW5ndGggPT09IDApIHtcbiAgICAgICAgICByZXR1cm4gdW5kZWZpbmVkO1xuICAgICAgICB9XG5cbiAgICAgICAgcmV0dXJuIGNvbnRlbnQubWFwKHRoaXMuc2VyaWFsaXNlLCB0aGlzKTtcbiAgICAgIH1cblxuICAgICAgcmV0dXJuIGNvbnRlbnQ7XG4gICAgfVxuICB9LCB7XG4gICAga2V5OiAnZGVzZXJpYWxpc2VDb250ZW50JyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gZGVzZXJpYWxpc2VDb250ZW50KGNvbnRlbnQpIHtcbiAgICAgIGlmIChjb250ZW50KSB7XG4gICAgICAgIGlmIChjb250ZW50LmVsZW1lbnQpIHtcbiAgICAgICAgICByZXR1cm4gdGhpcy5kZXNlcmlhbGlzZShjb250ZW50KTtcbiAgICAgICAgfVxuXG4gICAgICAgIGlmIChjb250ZW50LmtleSkge1xuICAgICAgICAgIHZhciBwYWlyID0gbmV3IHRoaXMubmFtZXNwYWNlLktleVZhbHVlUGFpcih0aGlzLmRlc2VyaWFsaXNlKGNvbnRlbnQua2V5KSk7XG5cbiAgICAgICAgICBpZiAoY29udGVudC52YWx1ZSkge1xuICAgICAgICAgICAgcGFpci52YWx1ZSA9IHRoaXMuZGVzZXJpYWxpc2UoY29udGVudC52YWx1ZSk7XG4gICAgICAgICAgfVxuXG4gICAgICAgICAgcmV0dXJuIHBhaXI7XG4gICAgICAgIH1cblxuICAgICAgICBpZiAoY29udGVudC5tYXApIHtcbiAgICAgICAgICByZXR1cm4gY29udGVudC5tYXAodGhpcy5kZXNlcmlhbGlzZSwgdGhpcyk7XG4gICAgICAgIH1cbiAgICAgIH1cblxuICAgICAgcmV0dXJuIGNvbnRlbnQ7XG4gICAgfVxuICB9LCB7XG4gICAga2V5OiAnc2VyaWFsaXNlT2JqZWN0JyxcbiAgICB2YWx1ZTogZnVuY3Rpb24gc2VyaWFsaXNlT2JqZWN0KG9iaikge1xuICAgICAgdmFyIF90aGlzID0gdGhpcztcblxuICAgICAgdmFyIHJlc3VsdCA9IHt9O1xuXG4gICAgICBvYmouZm9yRWFjaChmdW5jdGlvbiAodmFsdWUsIGtleSkge1xuICAgICAgICBpZiAodmFsdWUpIHtcbiAgICAgICAgICByZXN1bHRba2V5LnRvVmFsdWUoKV0gPSBfdGhpcy5zZXJpYWxpc2UodmFsdWUpO1xuICAgICAgICB9XG4gICAgICB9KTtcblxuICAgICAgaWYgKE9iamVjdC5rZXlzKHJlc3VsdCkubGVuZ3RoID09PSAwKSB7XG4gICAgICAgIHJldHVybiB1bmRlZmluZWQ7XG4gICAgICB9XG5cbiAgICAgIHJldHVybiByZXN1bHQ7XG4gICAgfVxuICB9LCB7XG4gICAga2V5OiAnZGVzZXJpYWxpc2VPYmplY3QnLFxuICAgIHZhbHVlOiBmdW5jdGlvbiBkZXNlcmlhbGlzZU9iamVjdChmcm9tLCB0bykge1xuICAgICAgdmFyIF90aGlzMiA9IHRoaXM7XG5cbiAgICAgIE9iamVjdC5rZXlzKGZyb20pLmZvckVhY2goZnVuY3Rpb24gKGtleSkge1xuICAgICAgICB0by5zZXQoa2V5LCBfdGhpczIuZGVzZXJpYWxpc2UoZnJvbVtrZXldKSk7XG4gICAgICB9KTtcbiAgICB9XG4gIH1dKTtcblxuICByZXR1cm4gSlNPTlNlcmlhbGlzZXI7XG59KCk7XG5cbm1vZHVsZS5leHBvcnRzID0gSlNPTlNlcmlhbGlzZXI7IiwidmFyIGdldE5hdGl2ZSA9IHJlcXVpcmUoJy4vX2dldE5hdGl2ZScpLFxuICAgIHJvb3QgPSByZXF1aXJlKCcuL19yb290Jyk7XG5cbi8qIEJ1aWx0LWluIG1ldGhvZCByZWZlcmVuY2VzIHRoYXQgYXJlIHZlcmlmaWVkIHRvIGJlIG5hdGl2ZS4gKi9cbnZhciBEYXRhVmlldyA9IGdldE5hdGl2ZShyb290LCAnRGF0YVZpZXcnKTtcblxubW9kdWxlLmV4cG9ydHMgPSBEYXRhVmlldztcbiIsInZhciBoYXNoQ2xlYXIgPSByZXF1aXJlKCcuL19oYXNoQ2xlYXInKSxcbiAgICBoYXNoRGVsZXRlID0gcmVxdWlyZSgnLi9faGFzaERlbGV0ZScpLFxuICAgIGhhc2hHZXQgPSByZXF1aXJlKCcuL19oYXNoR2V0JyksXG4gICAgaGFzaEhhcyA9IHJlcXVpcmUoJy4vX2hhc2hIYXMnKSxcbiAgICBoYXNoU2V0ID0gcmVxdWlyZSgnLi9faGFzaFNldCcpO1xuXG4vKipcbiAqIENyZWF0ZXMgYSBoYXNoIG9iamVjdC5cbiAqXG4gKiBAcHJpdmF0ZVxuICogQGNvbnN0cnVjdG9yXG4gKiBAcGFyYW0ge0FycmF5fSBbZW50cmllc10gVGhlIGtleS12YWx1ZSBwYWlycyB0byBjYWNoZS5cbiAqL1xuZnVuY3Rpb24gSGFzaChlbnRyaWVzKSB7XG4gIHZhciBpbmRleCA9IC0xLFxuICAgICAgbGVuZ3RoID0gZW50cmllcyA9PSBudWxsID8gMCA6IGVudHJpZXMubGVuZ3RoO1xuXG4gIHRoaXMuY2xlYXIoKTtcbiAgd2hpbGUgKCsraW5kZXggPCBsZW5ndGgpIHtcbiAgICB2YXIgZW50cnkgPSBlbnRyaWVzW2luZGV4XTtcbiAgICB0aGlzLnNldChlbnRyeVswXSwgZW50cnlbMV0pO1xuICB9XG59XG5cbi8vIEFkZCBtZXRob2RzIHRvIGBIYXNoYC5cbkhhc2gucHJvdG90eXBlLmNsZWFyID0gaGFzaENsZWFyO1xuSGFzaC5wcm90b3R5cGVbJ2RlbGV0ZSddID0gaGFzaERlbGV0ZTtcbkhhc2gucHJvdG90eXBlLmdldCA9IGhhc2hHZXQ7XG5IYXNoLnByb3RvdHlwZS5oYXMgPSBoYXNoSGFzO1xuSGFzaC5wcm90b3R5cGUuc2V0ID0gaGFzaFNldDtcblxubW9kdWxlLmV4cG9ydHMgPSBIYXNoO1xuIiwidmFyIGxpc3RDYWNoZUNsZWFyID0gcmVxdWlyZSgnLi9fbGlzdENhY2hlQ2xlYXInKSxcbiAgICBsaXN0Q2FjaGVEZWxldGUgPSByZXF1aXJlKCcuL19saXN0Q2FjaGVEZWxldGUnKSxcbiAgICBsaXN0Q2FjaGVHZXQgPSByZXF1aXJlKCcuL19saXN0Q2FjaGVHZXQnKSxcbiAgICBsaXN0Q2FjaGVIYXMgPSByZXF1aXJlKCcuL19saXN0Q2FjaGVIYXMnKSxcbiAgICBsaXN0Q2FjaGVTZXQgPSByZXF1aXJlKCcuL19saXN0Q2FjaGVTZXQnKTtcblxuLyoqXG4gKiBDcmVhdGVzIGFuIGxpc3QgY2FjaGUgb2JqZWN0LlxuICpcbiAqIEBwcml2YXRlXG4gKiBAY29uc3RydWN0b3JcbiAqIEBwYXJhbSB7QXJyYXl9IFtlbnRyaWVzXSBUaGUga2V5LXZhbHVlIHBhaXJzIHRvIGNhY2hlLlxuICovXG5mdW5jdGlvbiBMaXN0Q2FjaGUoZW50cmllcykge1xuICB2YXIgaW5kZXggPSAtMSxcbiAgICAgIGxlbmd0aCA9IGVudHJpZXMgPT0gbnVsbCA/IDAgOiBlbnRyaWVzLmxlbmd0aDtcblxuICB0aGlzLmNsZWFyKCk7XG4gIHdoaWxlICgrK2luZGV4IDwgbGVuZ3RoKSB7XG4gICAgdmFyIGVudHJ5ID0gZW50cmllc1tpbmRleF07XG4gICAgdGhpcy5zZXQoZW50cnlbMF0sIGVudHJ5WzFdKTtcbiAgfVxufVxuXG4vLyBBZGQgbWV0aG9kcyB0byBgTGlzdENhY2hlYC5cbkxpc3RDYWNoZS5wcm90b3R5cGUuY2xlYXIgPSBsaXN0Q2FjaGVDbGVhcjtcbkxpc3RDYWNoZS5wcm90b3R5cGVbJ2RlbGV0ZSddID0gbGlzdENhY2hlRGVsZXRlO1xuTGlzdENhY2hlLnByb3RvdHlwZS5nZXQgPSBsaXN0Q2FjaGVHZXQ7XG5MaXN0Q2FjaGUucHJvdG90eXBlLmhhcyA9IGxpc3RDYWNoZUhhcztcbkxpc3RDYWNoZS5wcm90b3R5cGUuc2V0ID0gbGlzdENhY2hlU2V0O1xuXG5tb2R1bGUuZXhwb3J0cyA9IExpc3RDYWNoZTtcbiIsInZhciBnZXROYXRpdmUgPSByZXF1aXJlKCcuL19nZXROYXRpdmUnKSxcbiAgICByb290ID0gcmVxdWlyZSgnLi9fcm9vdCcpO1xuXG4vKiBCdWlsdC1pbiBtZXRob2QgcmVmZXJlbmNlcyB0aGF0IGFyZSB2ZXJpZmllZCB0byBiZSBuYXRpdmUuICovXG52YXIgTWFwID0gZ2V0TmF0aXZlKHJvb3QsICdNYXAnKTtcblxubW9kdWxlLmV4cG9ydHMgPSBNYXA7XG4iLCJ2YXIgbWFwQ2FjaGVDbGVhciA9IHJlcXVpcmUoJy4vX21hcENhY2hlQ2xlYXInKSxcbiAgICBtYXBDYWNoZURlbGV0ZSA9IHJlcXVpcmUoJy4vX21hcENhY2hlRGVsZXRlJyksXG4gICAgbWFwQ2FjaGVHZXQgPSByZXF1aXJlKCcuL19tYXBDYWNoZUdldCcpLFxuICAgIG1hcENhY2hlSGFzID0gcmVxdWlyZSgnLi9fbWFwQ2FjaGVIYXMnKSxcbiAgICBtYXBDYWNoZVNldCA9IHJlcXVpcmUoJy4vX21hcENhY2hlU2V0Jyk7XG5cbi8qKlxuICogQ3JlYXRlcyBhIG1hcCBjYWNoZSBvYmplY3QgdG8gc3RvcmUga2V5LXZhbHVlIHBhaXJzLlxuICpcbiAqIEBwcml2YXRlXG4gKiBAY29uc3RydWN0b3JcbiAqIEBwYXJhbSB7QXJyYXl9IFtlbnRyaWVzXSBUaGUga2V5LXZhbHVlIHBhaXJzIHRvIGNhY2hlLlxuICovXG5mdW5jdGlvbiBNYXBDYWNoZShlbnRyaWVzKSB7XG4gIHZhciBpbmRleCA9IC0xLFxuICAgICAgbGVuZ3RoID0gZW50cmllcyA9PSBudWxsID8gMCA6IGVudHJpZXMubGVuZ3RoO1xuXG4gIHRoaXMuY2xlYXIoKTtcbiAgd2hpbGUgKCsraW5kZXggPCBsZW5ndGgpIHtcbiAgICB2YXIgZW50cnkgPSBlbnRyaWVzW2luZGV4XTtcbiAgICB0aGlzLnNldChlbnRyeVswXSwgZW50cnlbMV0pO1xuICB9XG59XG5cbi8vIEFkZCBtZXRob2RzIHRvIGBNYXBDYWNoZWAuXG5NYXBDYWNoZS5wcm90b3R5cGUuY2xlYXIgPSBtYXBDYWNoZUNsZWFyO1xuTWFwQ2FjaGUucHJvdG90eXBlWydkZWxldGUnXSA9IG1hcENhY2hlRGVsZXRlO1xuTWFwQ2FjaGUucHJvdG90eXBlLmdldCA9IG1hcENhY2hlR2V0O1xuTWFwQ2FjaGUucHJvdG90eXBlLmhhcyA9IG1hcENhY2hlSGFzO1xuTWFwQ2FjaGUucHJvdG90eXBlLnNldCA9IG1hcENhY2hlU2V0O1xuXG5tb2R1bGUuZXhwb3J0cyA9IE1hcENhY2hlO1xuIiwidmFyIGdldE5hdGl2ZSA9IHJlcXVpcmUoJy4vX2dldE5hdGl2ZScpLFxuICAgIHJvb3QgPSByZXF1aXJlKCcuL19yb290Jyk7XG5cbi8qIEJ1aWx0LWluIG1ldGhvZCByZWZlcmVuY2VzIHRoYXQgYXJlIHZlcmlmaWVkIHRvIGJlIG5hdGl2ZS4gKi9cbnZhciBQcm9taXNlID0gZ2V0TmF0aXZlKHJvb3QsICdQcm9taXNlJyk7XG5cbm1vZHVsZS5leHBvcnRzID0gUHJvbWlzZTtcbiIsInZhciBnZXROYXRpdmUgPSByZXF1aXJlKCcuL19nZXROYXRpdmUnKSxcbiAgICByb290ID0gcmVxdWlyZSgnLi9fcm9vdCcpO1xuXG4vKiBCdWlsdC1pbiBtZXRob2QgcmVmZXJlbmNlcyB0aGF0IGFyZSB2ZXJpZmllZCB0byBiZSBuYXRpdmUuICovXG52YXIgU2V0ID0gZ2V0TmF0aXZlKHJvb3QsICdTZXQnKTtcblxubW9kdWxlLmV4cG9ydHMgPSBTZXQ7XG4iLCJ2YXIgTWFwQ2FjaGUgPSByZXF1aXJlKCcuL19NYXBDYWNoZScpLFxuICAgIHNldENhY2hlQWRkID0gcmVxdWlyZSgnLi9fc2V0Q2FjaGVBZGQnKSxcbiAgICBzZXRDYWNoZUhhcyA9IHJlcXVpcmUoJy4vX3NldENhY2hlSGFzJyk7XG5cbi8qKlxuICpcbiAqIENyZWF0ZXMgYW4gYXJyYXkgY2FjaGUgb2JqZWN0IHRvIHN0b3JlIHVuaXF1ZSB2YWx1ZXMuXG4gKlxuICogQHByaXZhdGVcbiAqIEBjb25zdHJ1Y3RvclxuICogQHBhcmFtIHtBcnJheX0gW3ZhbHVlc10gVGhlIHZhbHVlcyB0byBjYWNoZS5cbiAqL1xuZnVuY3Rpb24gU2V0Q2FjaGUodmFsdWVzKSB7XG4gIHZhciBpbmRleCA9IC0xLFxuICAgICAgbGVuZ3RoID0gdmFsdWVzID09IG51bGwgPyAwIDogdmFsdWVzLmxlbmd0aDtcblxuICB0aGlzLl9fZGF0YV9fID0gbmV3IE1hcENhY2hlO1xuICB3aGlsZSAoKytpbmRleCA8IGxlbmd0aCkge1xuICAgIHRoaXMuYWRkKHZhbHVlc1tpbmRleF0pO1xuICB9XG59XG5cbi8vIEFkZCBtZXRob2RzIHRvIGBTZXRDYWNoZWAuXG5TZXRDYWNoZS5wcm90b3R5cGUuYWRkID0gU2V0Q2FjaGUucHJvdG90eXBlLnB1c2ggPSBzZXRDYWNoZUFkZDtcblNldENhY2hlLnByb3RvdHlwZS5oYXMgPSBzZXRDYWNoZUhhcztcblxubW9kdWxlLmV4cG9ydHMgPSBTZXRDYWNoZTtcbiIsInZhciBMaXN0Q2FjaGUgPSByZXF1aXJlKCcuL19MaXN0Q2FjaGUnKSxcbiAgICBzdGFja0NsZWFyID0gcmVxdWlyZSgnLi9fc3RhY2tDbGVhcicpLFxuICAgIHN0YWNrRGVsZXRlID0gcmVxdWlyZSgnLi9fc3RhY2tEZWxldGUnKSxcbiAgICBzdGFja0dldCA9IHJlcXVpcmUoJy4vX3N0YWNrR2V0JyksXG4gICAgc3RhY2tIYXMgPSByZXF1aXJlKCcuL19zdGFja0hhcycpLFxuICAgIHN0YWNrU2V0ID0gcmVxdWlyZSgnLi9fc3RhY2tTZXQnKTtcblxuLyoqXG4gKiBDcmVhdGVzIGEgc3RhY2sgY2FjaGUgb2JqZWN0IHRvIHN0b3JlIGtleS12YWx1ZSBwYWlycy5cbiAqXG4gKiBAcHJpdmF0ZVxuICogQGNvbnN0cnVjdG9yXG4gKiBAcGFyYW0ge0FycmF5fSBbZW50cmllc10gVGhlIGtleS12YWx1ZSBwYWlycyB0byBjYWNoZS5cbiAqL1xuZnVuY3Rpb24gU3RhY2soZW50cmllcykge1xuICB2YXIgZGF0YSA9IHRoaXMuX19kYXRhX18gPSBuZXcgTGlzdENhY2hlKGVudHJpZXMpO1xuICB0aGlzLnNpemUgPSBkYXRhLnNpemU7XG59XG5cbi8vIEFkZCBtZXRob2RzIHRvIGBTdGFja2AuXG5TdGFjay5wcm90b3R5cGUuY2xlYXIgPSBzdGFja0NsZWFyO1xuU3RhY2sucHJvdG90eXBlWydkZWxldGUnXSA9IHN0YWNrRGVsZXRlO1xuU3RhY2sucHJvdG90eXBlLmdldCA9IHN0YWNrR2V0O1xuU3RhY2sucHJvdG90eXBlLmhhcyA9IHN0YWNrSGFzO1xuU3RhY2sucHJvdG90eXBlLnNldCA9IHN0YWNrU2V0O1xuXG5tb2R1bGUuZXhwb3J0cyA9IFN0YWNrO1xuIiwidmFyIHJvb3QgPSByZXF1aXJlKCcuL19yb290Jyk7XG5cbi8qKiBCdWlsdC1pbiB2YWx1ZSByZWZlcmVuY2VzLiAqL1xudmFyIFN5bWJvbCA9IHJvb3QuU3ltYm9sO1xuXG5tb2R1bGUuZXhwb3J0cyA9IFN5bWJvbDtcbiIsInZhciByb290ID0gcmVxdWlyZSgnLi9fcm9vdCcpO1xuXG4vKiogQnVpbHQtaW4gdmFsdWUgcmVmZXJlbmNlcy4gKi9cbnZhciBVaW50OEFycmF5ID0gcm9vdC5VaW50OEFycmF5O1xuXG5tb2R1bGUuZXhwb3J0cyA9IFVpbnQ4QXJyYXk7XG4iLCJ2YXIgZ2V0TmF0aXZlID0gcmVxdWlyZSgnLi9fZ2V0TmF0aXZlJyksXG4gICAgcm9vdCA9IHJlcXVpcmUoJy4vX3Jvb3QnKTtcblxuLyogQnVpbHQtaW4gbWV0aG9kIHJlZmVyZW5jZXMgdGhhdCBhcmUgdmVyaWZpZWQgdG8gYmUgbmF0aXZlLiAqL1xudmFyIFdlYWtNYXAgPSBnZXROYXRpdmUocm9vdCwgJ1dlYWtNYXAnKTtcblxubW9kdWxlLmV4cG9ydHMgPSBXZWFrTWFwO1xuIiwiLyoqXG4gKiBBIHNwZWNpYWxpemVkIHZlcnNpb24gb2YgYF8uZmlsdGVyYCBmb3IgYXJyYXlzIHdpdGhvdXQgc3VwcG9ydCBmb3JcbiAqIGl0ZXJhdGVlIHNob3J0aGFuZHMuXG4gKlxuICogQHByaXZhdGVcbiAqIEBwYXJhbSB7QXJyYXl9IFthcnJheV0gVGhlIGFycmF5IHRvIGl0ZXJhdGUgb3Zlci5cbiAqIEBwYXJhbSB7RnVuY3Rpb259IHByZWRpY2F0ZSBUaGUgZnVuY3Rpb24gaW52b2tlZCBwZXIgaXRlcmF0aW9uLlxuICogQHJldHVybnMge0FycmF5fSBSZXR1cm5zIHRoZSBuZXcgZmlsdGVyZWQgYXJyYXkuXG4gKi9cbmZ1bmN0aW9uIGFycmF5RmlsdGVyKGFycmF5LCBwcmVkaWNhdGUpIHtcbiAgdmFyIGluZGV4ID0gLTEsXG4gICAgICBsZW5ndGggPSBhcnJheSA9PSBudWxsID8gMCA6IGFycmF5Lmxlbmd0aCxcbiAgICAgIHJlc0luZGV4ID0gMCxcbiAgICAgIHJlc3VsdCA9IFtdO1xuXG4gIHdoaWxlICgrK2luZGV4IDwgbGVuZ3RoKSB7XG4gICAgdmFyIHZhbHVlID0gYXJyYXlbaW5kZXhdO1xuICAgIGlmIChwcmVkaWNhdGUodmFsdWUsIGluZGV4LCBhcnJheSkpIHtcbiAgICAgIHJlc3VsdFtyZXNJbmRleCsrXSA9IHZhbHVlO1xuICAgIH1cbiAgfVxuICByZXR1cm4gcmVzdWx0O1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IGFycmF5RmlsdGVyO1xuIiwidmFyIGJhc2VUaW1lcyA9IHJlcXVpcmUoJy4vX2Jhc2VUaW1lcycpLFxuICAgIGlzQXJndW1lbnRzID0gcmVxdWlyZSgnLi9pc0FyZ3VtZW50cycpLFxuICAgIGlzQXJyYXkgPSByZXF1aXJlKCcuL2lzQXJyYXknKSxcbiAgICBpc0J1ZmZlciA9IHJlcXVpcmUoJy4vaXNCdWZmZXInKSxcbiAgICBpc0luZGV4ID0gcmVxdWlyZSgnLi9faXNJbmRleCcpLFxuICAgIGlzVHlwZWRBcnJheSA9IHJlcXVpcmUoJy4vaXNUeXBlZEFycmF5Jyk7XG5cbi8qKiBVc2VkIGZvciBidWlsdC1pbiBtZXRob2QgcmVmZXJlbmNlcy4gKi9cbnZhciBvYmplY3RQcm90byA9IE9iamVjdC5wcm90b3R5cGU7XG5cbi8qKiBVc2VkIHRvIGNoZWNrIG9iamVjdHMgZm9yIG93biBwcm9wZXJ0aWVzLiAqL1xudmFyIGhhc093blByb3BlcnR5ID0gb2JqZWN0UHJvdG8uaGFzT3duUHJvcGVydHk7XG5cbi8qKlxuICogQ3JlYXRlcyBhbiBhcnJheSBvZiB0aGUgZW51bWVyYWJsZSBwcm9wZXJ0eSBuYW1lcyBvZiB0aGUgYXJyYXktbGlrZSBgdmFsdWVgLlxuICpcbiAqIEBwcml2YXRlXG4gKiBAcGFyYW0geyp9IHZhbHVlIFRoZSB2YWx1ZSB0byBxdWVyeS5cbiAqIEBwYXJhbSB7Ym9vbGVhbn0gaW5oZXJpdGVkIFNwZWNpZnkgcmV0dXJuaW5nIGluaGVyaXRlZCBwcm9wZXJ0eSBuYW1lcy5cbiAqIEByZXR1cm5zIHtBcnJheX0gUmV0dXJucyB0aGUgYXJyYXkgb2YgcHJvcGVydHkgbmFtZXMuXG4gKi9cbmZ1bmN0aW9uIGFycmF5TGlrZUtleXModmFsdWUsIGluaGVyaXRlZCkge1xuICB2YXIgaXNBcnIgPSBpc0FycmF5KHZhbHVlKSxcbiAgICAgIGlzQXJnID0gIWlzQXJyICYmIGlzQXJndW1lbnRzKHZhbHVlKSxcbiAgICAgIGlzQnVmZiA9ICFpc0FyciAmJiAhaXNBcmcgJiYgaXNCdWZmZXIodmFsdWUpLFxuICAgICAgaXNUeXBlID0gIWlzQXJyICYmICFpc0FyZyAmJiAhaXNCdWZmICYmIGlzVHlwZWRBcnJheSh2YWx1ZSksXG4gICAgICBza2lwSW5kZXhlcyA9IGlzQXJyIHx8IGlzQXJnIHx8IGlzQnVmZiB8fCBpc1R5cGUsXG4gICAgICByZXN1bHQgPSBza2lwSW5kZXhlcyA/IGJhc2VUaW1lcyh2YWx1ZS5sZW5ndGgsIFN0cmluZykgOiBbXSxcbiAgICAgIGxlbmd0aCA9IHJlc3VsdC5sZW5ndGg7XG5cbiAgZm9yICh2YXIga2V5IGluIHZhbHVlKSB7XG4gICAgaWYgKChpbmhlcml0ZWQgfHwgaGFzT3duUHJvcGVydHkuY2FsbCh2YWx1ZSwga2V5KSkgJiZcbiAgICAgICAgIShza2lwSW5kZXhlcyAmJiAoXG4gICAgICAgICAgIC8vIFNhZmFyaSA5IGhhcyBlbnVtZXJhYmxlIGBhcmd1bWVudHMubGVuZ3RoYCBpbiBzdHJpY3QgbW9kZS5cbiAgICAgICAgICAga2V5ID09ICdsZW5ndGgnIHx8XG4gICAgICAgICAgIC8vIE5vZGUuanMgMC4xMCBoYXMgZW51bWVyYWJsZSBub24taW5kZXggcHJvcGVydGllcyBvbiBidWZmZXJzLlxuICAgICAgICAgICAoaXNCdWZmICYmIChrZXkgPT0gJ29mZnNldCcgfHwga2V5ID09ICdwYXJlbnQnKSkgfHxcbiAgICAgICAgICAgLy8gUGhhbnRvbUpTIDIgaGFzIGVudW1lcmFibGUgbm9uLWluZGV4IHByb3BlcnRpZXMgb24gdHlwZWQgYXJyYXlzLlxuICAgICAgICAgICAoaXNUeXBlICYmIChrZXkgPT0gJ2J1ZmZlcicgfHwga2V5ID09ICdieXRlTGVuZ3RoJyB8fCBrZXkgPT0gJ2J5dGVPZmZzZXQnKSkgfHxcbiAgICAgICAgICAgLy8gU2tpcCBpbmRleCBwcm9wZXJ0aWVzLlxuICAgICAgICAgICBpc0luZGV4KGtleSwgbGVuZ3RoKVxuICAgICAgICApKSkge1xuICAgICAgcmVzdWx0LnB1c2goa2V5KTtcbiAgICB9XG4gIH1cbiAgcmV0dXJuIHJlc3VsdDtcbn1cblxubW9kdWxlLmV4cG9ydHMgPSBhcnJheUxpa2VLZXlzO1xuIiwiLyoqXG4gKiBBcHBlbmRzIHRoZSBlbGVtZW50cyBvZiBgdmFsdWVzYCB0byBgYXJyYXlgLlxuICpcbiAqIEBwcml2YXRlXG4gKiBAcGFyYW0ge0FycmF5fSBhcnJheSBUaGUgYXJyYXkgdG8gbW9kaWZ5LlxuICogQHBhcmFtIHtBcnJheX0gdmFsdWVzIFRoZSB2YWx1ZXMgdG8gYXBwZW5kLlxuICogQHJldHVybnMge0FycmF5fSBSZXR1cm5zIGBhcnJheWAuXG4gKi9cbmZ1bmN0aW9uIGFycmF5UHVzaChhcnJheSwgdmFsdWVzKSB7XG4gIHZhciBpbmRleCA9IC0xLFxuICAgICAgbGVuZ3RoID0gdmFsdWVzLmxlbmd0aCxcbiAgICAgIG9mZnNldCA9IGFycmF5Lmxlbmd0aDtcblxuICB3aGlsZSAoKytpbmRleCA8IGxlbmd0aCkge1xuICAgIGFycmF5W29mZnNldCArIGluZGV4XSA9IHZhbHVlc1tpbmRleF07XG4gIH1cbiAgcmV0dXJuIGFycmF5O1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IGFycmF5UHVzaDtcbiIsIi8qKlxuICogQSBzcGVjaWFsaXplZCB2ZXJzaW9uIG9mIGBfLnNvbWVgIGZvciBhcnJheXMgd2l0aG91dCBzdXBwb3J0IGZvciBpdGVyYXRlZVxuICogc2hvcnRoYW5kcy5cbiAqXG4gKiBAcHJpdmF0ZVxuICogQHBhcmFtIHtBcnJheX0gW2FycmF5XSBUaGUgYXJyYXkgdG8gaXRlcmF0ZSBvdmVyLlxuICogQHBhcmFtIHtGdW5jdGlvbn0gcHJlZGljYXRlIFRoZSBmdW5jdGlvbiBpbnZva2VkIHBlciBpdGVyYXRpb24uXG4gKiBAcmV0dXJucyB7Ym9vbGVhbn0gUmV0dXJucyBgdHJ1ZWAgaWYgYW55IGVsZW1lbnQgcGFzc2VzIHRoZSBwcmVkaWNhdGUgY2hlY2ssXG4gKiAgZWxzZSBgZmFsc2VgLlxuICovXG5mdW5jdGlvbiBhcnJheVNvbWUoYXJyYXksIHByZWRpY2F0ZSkge1xuICB2YXIgaW5kZXggPSAtMSxcbiAgICAgIGxlbmd0aCA9IGFycmF5ID09IG51bGwgPyAwIDogYXJyYXkubGVuZ3RoO1xuXG4gIHdoaWxlICgrK2luZGV4IDwgbGVuZ3RoKSB7XG4gICAgaWYgKHByZWRpY2F0ZShhcnJheVtpbmRleF0sIGluZGV4LCBhcnJheSkpIHtcbiAgICAgIHJldHVybiB0cnVlO1xuICAgIH1cbiAgfVxuICByZXR1cm4gZmFsc2U7XG59XG5cbm1vZHVsZS5leHBvcnRzID0gYXJyYXlTb21lO1xuIiwidmFyIGVxID0gcmVxdWlyZSgnLi9lcScpO1xuXG4vKipcbiAqIEdldHMgdGhlIGluZGV4IGF0IHdoaWNoIHRoZSBga2V5YCBpcyBmb3VuZCBpbiBgYXJyYXlgIG9mIGtleS12YWx1ZSBwYWlycy5cbiAqXG4gKiBAcHJpdmF0ZVxuICogQHBhcmFtIHtBcnJheX0gYXJyYXkgVGhlIGFycmF5IHRvIGluc3BlY3QuXG4gKiBAcGFyYW0geyp9IGtleSBUaGUga2V5IHRvIHNlYXJjaCBmb3IuXG4gKiBAcmV0dXJucyB7bnVtYmVyfSBSZXR1cm5zIHRoZSBpbmRleCBvZiB0aGUgbWF0Y2hlZCB2YWx1ZSwgZWxzZSBgLTFgLlxuICovXG5mdW5jdGlvbiBhc3NvY0luZGV4T2YoYXJyYXksIGtleSkge1xuICB2YXIgbGVuZ3RoID0gYXJyYXkubGVuZ3RoO1xuICB3aGlsZSAobGVuZ3RoLS0pIHtcbiAgICBpZiAoZXEoYXJyYXlbbGVuZ3RoXVswXSwga2V5KSkge1xuICAgICAgcmV0dXJuIGxlbmd0aDtcbiAgICB9XG4gIH1cbiAgcmV0dXJuIC0xO1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IGFzc29jSW5kZXhPZjtcbiIsInZhciBhcnJheVB1c2ggPSByZXF1aXJlKCcuL19hcnJheVB1c2gnKSxcbiAgICBpc0FycmF5ID0gcmVxdWlyZSgnLi9pc0FycmF5Jyk7XG5cbi8qKlxuICogVGhlIGJhc2UgaW1wbGVtZW50YXRpb24gb2YgYGdldEFsbEtleXNgIGFuZCBgZ2V0QWxsS2V5c0luYCB3aGljaCB1c2VzXG4gKiBga2V5c0Z1bmNgIGFuZCBgc3ltYm9sc0Z1bmNgIHRvIGdldCB0aGUgZW51bWVyYWJsZSBwcm9wZXJ0eSBuYW1lcyBhbmRcbiAqIHN5bWJvbHMgb2YgYG9iamVjdGAuXG4gKlxuICogQHByaXZhdGVcbiAqIEBwYXJhbSB7T2JqZWN0fSBvYmplY3QgVGhlIG9iamVjdCB0byBxdWVyeS5cbiAqIEBwYXJhbSB7RnVuY3Rpb259IGtleXNGdW5jIFRoZSBmdW5jdGlvbiB0byBnZXQgdGhlIGtleXMgb2YgYG9iamVjdGAuXG4gKiBAcGFyYW0ge0Z1bmN0aW9ufSBzeW1ib2xzRnVuYyBUaGUgZnVuY3Rpb24gdG8gZ2V0IHRoZSBzeW1ib2xzIG9mIGBvYmplY3RgLlxuICogQHJldHVybnMge0FycmF5fSBSZXR1cm5zIHRoZSBhcnJheSBvZiBwcm9wZXJ0eSBuYW1lcyBhbmQgc3ltYm9scy5cbiAqL1xuZnVuY3Rpb24gYmFzZUdldEFsbEtleXMob2JqZWN0LCBrZXlzRnVuYywgc3ltYm9sc0Z1bmMpIHtcbiAgdmFyIHJlc3VsdCA9IGtleXNGdW5jKG9iamVjdCk7XG4gIHJldHVybiBpc0FycmF5KG9iamVjdCkgPyByZXN1bHQgOiBhcnJheVB1c2gocmVzdWx0LCBzeW1ib2xzRnVuYyhvYmplY3QpKTtcbn1cblxubW9kdWxlLmV4cG9ydHMgPSBiYXNlR2V0QWxsS2V5cztcbiIsInZhciBTeW1ib2wgPSByZXF1aXJlKCcuL19TeW1ib2wnKSxcbiAgICBnZXRSYXdUYWcgPSByZXF1aXJlKCcuL19nZXRSYXdUYWcnKSxcbiAgICBvYmplY3RUb1N0cmluZyA9IHJlcXVpcmUoJy4vX29iamVjdFRvU3RyaW5nJyk7XG5cbi8qKiBgT2JqZWN0I3RvU3RyaW5nYCByZXN1bHQgcmVmZXJlbmNlcy4gKi9cbnZhciBudWxsVGFnID0gJ1tvYmplY3QgTnVsbF0nLFxuICAgIHVuZGVmaW5lZFRhZyA9ICdbb2JqZWN0IFVuZGVmaW5lZF0nO1xuXG4vKiogQnVpbHQtaW4gdmFsdWUgcmVmZXJlbmNlcy4gKi9cbnZhciBzeW1Ub1N0cmluZ1RhZyA9IFN5bWJvbCA/IFN5bWJvbC50b1N0cmluZ1RhZyA6IHVuZGVmaW5lZDtcblxuLyoqXG4gKiBUaGUgYmFzZSBpbXBsZW1lbnRhdGlvbiBvZiBgZ2V0VGFnYCB3aXRob3V0IGZhbGxiYWNrcyBmb3IgYnVnZ3kgZW52aXJvbm1lbnRzLlxuICpcbiAqIEBwcml2YXRlXG4gKiBAcGFyYW0geyp9IHZhbHVlIFRoZSB2YWx1ZSB0byBxdWVyeS5cbiAqIEByZXR1cm5zIHtzdHJpbmd9IFJldHVybnMgdGhlIGB0b1N0cmluZ1RhZ2AuXG4gKi9cbmZ1bmN0aW9uIGJhc2VHZXRUYWcodmFsdWUpIHtcbiAgaWYgKHZhbHVlID09IG51bGwpIHtcbiAgICByZXR1cm4gdmFsdWUgPT09IHVuZGVmaW5lZCA/IHVuZGVmaW5lZFRhZyA6IG51bGxUYWc7XG4gIH1cbiAgcmV0dXJuIChzeW1Ub1N0cmluZ1RhZyAmJiBzeW1Ub1N0cmluZ1RhZyBpbiBPYmplY3QodmFsdWUpKVxuICAgID8gZ2V0UmF3VGFnKHZhbHVlKVxuICAgIDogb2JqZWN0VG9TdHJpbmcodmFsdWUpO1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IGJhc2VHZXRUYWc7XG4iLCJ2YXIgYmFzZUdldFRhZyA9IHJlcXVpcmUoJy4vX2Jhc2VHZXRUYWcnKSxcbiAgICBpc09iamVjdExpa2UgPSByZXF1aXJlKCcuL2lzT2JqZWN0TGlrZScpO1xuXG4vKiogYE9iamVjdCN0b1N0cmluZ2AgcmVzdWx0IHJlZmVyZW5jZXMuICovXG52YXIgYXJnc1RhZyA9ICdbb2JqZWN0IEFyZ3VtZW50c10nO1xuXG4vKipcbiAqIFRoZSBiYXNlIGltcGxlbWVudGF0aW9uIG9mIGBfLmlzQXJndW1lbnRzYC5cbiAqXG4gKiBAcHJpdmF0ZVxuICogQHBhcmFtIHsqfSB2YWx1ZSBUaGUgdmFsdWUgdG8gY2hlY2suXG4gKiBAcmV0dXJucyB7Ym9vbGVhbn0gUmV0dXJucyBgdHJ1ZWAgaWYgYHZhbHVlYCBpcyBhbiBgYXJndW1lbnRzYCBvYmplY3QsXG4gKi9cbmZ1bmN0aW9uIGJhc2VJc0FyZ3VtZW50cyh2YWx1ZSkge1xuICByZXR1cm4gaXNPYmplY3RMaWtlKHZhbHVlKSAmJiBiYXNlR2V0VGFnKHZhbHVlKSA9PSBhcmdzVGFnO1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IGJhc2VJc0FyZ3VtZW50cztcbiIsInZhciBiYXNlSXNFcXVhbERlZXAgPSByZXF1aXJlKCcuL19iYXNlSXNFcXVhbERlZXAnKSxcbiAgICBpc09iamVjdExpa2UgPSByZXF1aXJlKCcuL2lzT2JqZWN0TGlrZScpO1xuXG4vKipcbiAqIFRoZSBiYXNlIGltcGxlbWVudGF0aW9uIG9mIGBfLmlzRXF1YWxgIHdoaWNoIHN1cHBvcnRzIHBhcnRpYWwgY29tcGFyaXNvbnNcbiAqIGFuZCB0cmFja3MgdHJhdmVyc2VkIG9iamVjdHMuXG4gKlxuICogQHByaXZhdGVcbiAqIEBwYXJhbSB7Kn0gdmFsdWUgVGhlIHZhbHVlIHRvIGNvbXBhcmUuXG4gKiBAcGFyYW0geyp9IG90aGVyIFRoZSBvdGhlciB2YWx1ZSB0byBjb21wYXJlLlxuICogQHBhcmFtIHtib29sZWFufSBiaXRtYXNrIFRoZSBiaXRtYXNrIGZsYWdzLlxuICogIDEgLSBVbm9yZGVyZWQgY29tcGFyaXNvblxuICogIDIgLSBQYXJ0aWFsIGNvbXBhcmlzb25cbiAqIEBwYXJhbSB7RnVuY3Rpb259IFtjdXN0b21pemVyXSBUaGUgZnVuY3Rpb24gdG8gY3VzdG9taXplIGNvbXBhcmlzb25zLlxuICogQHBhcmFtIHtPYmplY3R9IFtzdGFja10gVHJhY2tzIHRyYXZlcnNlZCBgdmFsdWVgIGFuZCBgb3RoZXJgIG9iamVjdHMuXG4gKiBAcmV0dXJucyB7Ym9vbGVhbn0gUmV0dXJucyBgdHJ1ZWAgaWYgdGhlIHZhbHVlcyBhcmUgZXF1aXZhbGVudCwgZWxzZSBgZmFsc2VgLlxuICovXG5mdW5jdGlvbiBiYXNlSXNFcXVhbCh2YWx1ZSwgb3RoZXIsIGJpdG1hc2ssIGN1c3RvbWl6ZXIsIHN0YWNrKSB7XG4gIGlmICh2YWx1ZSA9PT0gb3RoZXIpIHtcbiAgICByZXR1cm4gdHJ1ZTtcbiAgfVxuICBpZiAodmFsdWUgPT0gbnVsbCB8fCBvdGhlciA9PSBudWxsIHx8ICghaXNPYmplY3RMaWtlKHZhbHVlKSAmJiAhaXNPYmplY3RMaWtlKG90aGVyKSkpIHtcbiAgICByZXR1cm4gdmFsdWUgIT09IHZhbHVlICYmIG90aGVyICE9PSBvdGhlcjtcbiAgfVxuICByZXR1cm4gYmFzZUlzRXF1YWxEZWVwKHZhbHVlLCBvdGhlciwgYml0bWFzaywgY3VzdG9taXplciwgYmFzZUlzRXF1YWwsIHN0YWNrKTtcbn1cblxubW9kdWxlLmV4cG9ydHMgPSBiYXNlSXNFcXVhbDtcbiIsInZhciBTdGFjayA9IHJlcXVpcmUoJy4vX1N0YWNrJyksXG4gICAgZXF1YWxBcnJheXMgPSByZXF1aXJlKCcuL19lcXVhbEFycmF5cycpLFxuICAgIGVxdWFsQnlUYWcgPSByZXF1aXJlKCcuL19lcXVhbEJ5VGFnJyksXG4gICAgZXF1YWxPYmplY3RzID0gcmVxdWlyZSgnLi9fZXF1YWxPYmplY3RzJyksXG4gICAgZ2V0VGFnID0gcmVxdWlyZSgnLi9fZ2V0VGFnJyksXG4gICAgaXNBcnJheSA9IHJlcXVpcmUoJy4vaXNBcnJheScpLFxuICAgIGlzQnVmZmVyID0gcmVxdWlyZSgnLi9pc0J1ZmZlcicpLFxuICAgIGlzVHlwZWRBcnJheSA9IHJlcXVpcmUoJy4vaXNUeXBlZEFycmF5Jyk7XG5cbi8qKiBVc2VkIHRvIGNvbXBvc2UgYml0bWFza3MgZm9yIHZhbHVlIGNvbXBhcmlzb25zLiAqL1xudmFyIENPTVBBUkVfUEFSVElBTF9GTEFHID0gMTtcblxuLyoqIGBPYmplY3QjdG9TdHJpbmdgIHJlc3VsdCByZWZlcmVuY2VzLiAqL1xudmFyIGFyZ3NUYWcgPSAnW29iamVjdCBBcmd1bWVudHNdJyxcbiAgICBhcnJheVRhZyA9ICdbb2JqZWN0IEFycmF5XScsXG4gICAgb2JqZWN0VGFnID0gJ1tvYmplY3QgT2JqZWN0XSc7XG5cbi8qKiBVc2VkIGZvciBidWlsdC1pbiBtZXRob2QgcmVmZXJlbmNlcy4gKi9cbnZhciBvYmplY3RQcm90byA9IE9iamVjdC5wcm90b3R5cGU7XG5cbi8qKiBVc2VkIHRvIGNoZWNrIG9iamVjdHMgZm9yIG93biBwcm9wZXJ0aWVzLiAqL1xudmFyIGhhc093blByb3BlcnR5ID0gb2JqZWN0UHJvdG8uaGFzT3duUHJvcGVydHk7XG5cbi8qKlxuICogQSBzcGVjaWFsaXplZCB2ZXJzaW9uIG9mIGBiYXNlSXNFcXVhbGAgZm9yIGFycmF5cyBhbmQgb2JqZWN0cyB3aGljaCBwZXJmb3Jtc1xuICogZGVlcCBjb21wYXJpc29ucyBhbmQgdHJhY2tzIHRyYXZlcnNlZCBvYmplY3RzIGVuYWJsaW5nIG9iamVjdHMgd2l0aCBjaXJjdWxhclxuICogcmVmZXJlbmNlcyB0byBiZSBjb21wYXJlZC5cbiAqXG4gKiBAcHJpdmF0ZVxuICogQHBhcmFtIHtPYmplY3R9IG9iamVjdCBUaGUgb2JqZWN0IHRvIGNvbXBhcmUuXG4gKiBAcGFyYW0ge09iamVjdH0gb3RoZXIgVGhlIG90aGVyIG9iamVjdCB0byBjb21wYXJlLlxuICogQHBhcmFtIHtudW1iZXJ9IGJpdG1hc2sgVGhlIGJpdG1hc2sgZmxhZ3MuIFNlZSBgYmFzZUlzRXF1YWxgIGZvciBtb3JlIGRldGFpbHMuXG4gKiBAcGFyYW0ge0Z1bmN0aW9ufSBjdXN0b21pemVyIFRoZSBmdW5jdGlvbiB0byBjdXN0b21pemUgY29tcGFyaXNvbnMuXG4gKiBAcGFyYW0ge0Z1bmN0aW9ufSBlcXVhbEZ1bmMgVGhlIGZ1bmN0aW9uIHRvIGRldGVybWluZSBlcXVpdmFsZW50cyBvZiB2YWx1ZXMuXG4gKiBAcGFyYW0ge09iamVjdH0gW3N0YWNrXSBUcmFja3MgdHJhdmVyc2VkIGBvYmplY3RgIGFuZCBgb3RoZXJgIG9iamVjdHMuXG4gKiBAcmV0dXJucyB7Ym9vbGVhbn0gUmV0dXJucyBgdHJ1ZWAgaWYgdGhlIG9iamVjdHMgYXJlIGVxdWl2YWxlbnQsIGVsc2UgYGZhbHNlYC5cbiAqL1xuZnVuY3Rpb24gYmFzZUlzRXF1YWxEZWVwKG9iamVjdCwgb3RoZXIsIGJpdG1hc2ssIGN1c3RvbWl6ZXIsIGVxdWFsRnVuYywgc3RhY2spIHtcbiAgdmFyIG9iaklzQXJyID0gaXNBcnJheShvYmplY3QpLFxuICAgICAgb3RoSXNBcnIgPSBpc0FycmF5KG90aGVyKSxcbiAgICAgIG9ialRhZyA9IG9iaklzQXJyID8gYXJyYXlUYWcgOiBnZXRUYWcob2JqZWN0KSxcbiAgICAgIG90aFRhZyA9IG90aElzQXJyID8gYXJyYXlUYWcgOiBnZXRUYWcob3RoZXIpO1xuXG4gIG9ialRhZyA9IG9ialRhZyA9PSBhcmdzVGFnID8gb2JqZWN0VGFnIDogb2JqVGFnO1xuICBvdGhUYWcgPSBvdGhUYWcgPT0gYXJnc1RhZyA/IG9iamVjdFRhZyA6IG90aFRhZztcblxuICB2YXIgb2JqSXNPYmogPSBvYmpUYWcgPT0gb2JqZWN0VGFnLFxuICAgICAgb3RoSXNPYmogPSBvdGhUYWcgPT0gb2JqZWN0VGFnLFxuICAgICAgaXNTYW1lVGFnID0gb2JqVGFnID09IG90aFRhZztcblxuICBpZiAoaXNTYW1lVGFnICYmIGlzQnVmZmVyKG9iamVjdCkpIHtcbiAgICBpZiAoIWlzQnVmZmVyKG90aGVyKSkge1xuICAgICAgcmV0dXJuIGZhbHNlO1xuICAgIH1cbiAgICBvYmpJc0FyciA9IHRydWU7XG4gICAgb2JqSXNPYmogPSBmYWxzZTtcbiAgfVxuICBpZiAoaXNTYW1lVGFnICYmICFvYmpJc09iaikge1xuICAgIHN0YWNrIHx8IChzdGFjayA9IG5ldyBTdGFjayk7XG4gICAgcmV0dXJuIChvYmpJc0FyciB8fCBpc1R5cGVkQXJyYXkob2JqZWN0KSlcbiAgICAgID8gZXF1YWxBcnJheXMob2JqZWN0LCBvdGhlciwgYml0bWFzaywgY3VzdG9taXplciwgZXF1YWxGdW5jLCBzdGFjaylcbiAgICAgIDogZXF1YWxCeVRhZyhvYmplY3QsIG90aGVyLCBvYmpUYWcsIGJpdG1hc2ssIGN1c3RvbWl6ZXIsIGVxdWFsRnVuYywgc3RhY2spO1xuICB9XG4gIGlmICghKGJpdG1hc2sgJiBDT01QQVJFX1BBUlRJQUxfRkxBRykpIHtcbiAgICB2YXIgb2JqSXNXcmFwcGVkID0gb2JqSXNPYmogJiYgaGFzT3duUHJvcGVydHkuY2FsbChvYmplY3QsICdfX3dyYXBwZWRfXycpLFxuICAgICAgICBvdGhJc1dyYXBwZWQgPSBvdGhJc09iaiAmJiBoYXNPd25Qcm9wZXJ0eS5jYWxsKG90aGVyLCAnX193cmFwcGVkX18nKTtcblxuICAgIGlmIChvYmpJc1dyYXBwZWQgfHwgb3RoSXNXcmFwcGVkKSB7XG4gICAgICB2YXIgb2JqVW53cmFwcGVkID0gb2JqSXNXcmFwcGVkID8gb2JqZWN0LnZhbHVlKCkgOiBvYmplY3QsXG4gICAgICAgICAgb3RoVW53cmFwcGVkID0gb3RoSXNXcmFwcGVkID8gb3RoZXIudmFsdWUoKSA6IG90aGVyO1xuXG4gICAgICBzdGFjayB8fCAoc3RhY2sgPSBuZXcgU3RhY2spO1xuICAgICAgcmV0dXJuIGVxdWFsRnVuYyhvYmpVbndyYXBwZWQsIG90aFVud3JhcHBlZCwgYml0bWFzaywgY3VzdG9taXplciwgc3RhY2spO1xuICAgIH1cbiAgfVxuICBpZiAoIWlzU2FtZVRhZykge1xuICAgIHJldHVybiBmYWxzZTtcbiAgfVxuICBzdGFjayB8fCAoc3RhY2sgPSBuZXcgU3RhY2spO1xuICByZXR1cm4gZXF1YWxPYmplY3RzKG9iamVjdCwgb3RoZXIsIGJpdG1hc2ssIGN1c3RvbWl6ZXIsIGVxdWFsRnVuYywgc3RhY2spO1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IGJhc2VJc0VxdWFsRGVlcDtcbiIsInZhciBpc0Z1bmN0aW9uID0gcmVxdWlyZSgnLi9pc0Z1bmN0aW9uJyksXG4gICAgaXNNYXNrZWQgPSByZXF1aXJlKCcuL19pc01hc2tlZCcpLFxuICAgIGlzT2JqZWN0ID0gcmVxdWlyZSgnLi9pc09iamVjdCcpLFxuICAgIHRvU291cmNlID0gcmVxdWlyZSgnLi9fdG9Tb3VyY2UnKTtcblxuLyoqXG4gKiBVc2VkIHRvIG1hdGNoIGBSZWdFeHBgXG4gKiBbc3ludGF4IGNoYXJhY3RlcnNdKGh0dHA6Ly9lY21hLWludGVybmF0aW9uYWwub3JnL2VjbWEtMjYyLzcuMC8jc2VjLXBhdHRlcm5zKS5cbiAqL1xudmFyIHJlUmVnRXhwQ2hhciA9IC9bXFxcXF4kLiorPygpW1xcXXt9fF0vZztcblxuLyoqIFVzZWQgdG8gZGV0ZWN0IGhvc3QgY29uc3RydWN0b3JzIChTYWZhcmkpLiAqL1xudmFyIHJlSXNIb3N0Q3RvciA9IC9eXFxbb2JqZWN0IC4rP0NvbnN0cnVjdG9yXFxdJC87XG5cbi8qKiBVc2VkIGZvciBidWlsdC1pbiBtZXRob2QgcmVmZXJlbmNlcy4gKi9cbnZhciBmdW5jUHJvdG8gPSBGdW5jdGlvbi5wcm90b3R5cGUsXG4gICAgb2JqZWN0UHJvdG8gPSBPYmplY3QucHJvdG90eXBlO1xuXG4vKiogVXNlZCB0byByZXNvbHZlIHRoZSBkZWNvbXBpbGVkIHNvdXJjZSBvZiBmdW5jdGlvbnMuICovXG52YXIgZnVuY1RvU3RyaW5nID0gZnVuY1Byb3RvLnRvU3RyaW5nO1xuXG4vKiogVXNlZCB0byBjaGVjayBvYmplY3RzIGZvciBvd24gcHJvcGVydGllcy4gKi9cbnZhciBoYXNPd25Qcm9wZXJ0eSA9IG9iamVjdFByb3RvLmhhc093blByb3BlcnR5O1xuXG4vKiogVXNlZCB0byBkZXRlY3QgaWYgYSBtZXRob2QgaXMgbmF0aXZlLiAqL1xudmFyIHJlSXNOYXRpdmUgPSBSZWdFeHAoJ14nICtcbiAgZnVuY1RvU3RyaW5nLmNhbGwoaGFzT3duUHJvcGVydHkpLnJlcGxhY2UocmVSZWdFeHBDaGFyLCAnXFxcXCQmJylcbiAgLnJlcGxhY2UoL2hhc093blByb3BlcnR5fChmdW5jdGlvbikuKj8oPz1cXFxcXFwoKXwgZm9yIC4rPyg/PVxcXFxcXF0pL2csICckMS4qPycpICsgJyQnXG4pO1xuXG4vKipcbiAqIFRoZSBiYXNlIGltcGxlbWVudGF0aW9uIG9mIGBfLmlzTmF0aXZlYCB3aXRob3V0IGJhZCBzaGltIGNoZWNrcy5cbiAqXG4gKiBAcHJpdmF0ZVxuICogQHBhcmFtIHsqfSB2YWx1ZSBUaGUgdmFsdWUgdG8gY2hlY2suXG4gKiBAcmV0dXJucyB7Ym9vbGVhbn0gUmV0dXJucyBgdHJ1ZWAgaWYgYHZhbHVlYCBpcyBhIG5hdGl2ZSBmdW5jdGlvbixcbiAqICBlbHNlIGBmYWxzZWAuXG4gKi9cbmZ1bmN0aW9uIGJhc2VJc05hdGl2ZSh2YWx1ZSkge1xuICBpZiAoIWlzT2JqZWN0KHZhbHVlKSB8fCBpc01hc2tlZCh2YWx1ZSkpIHtcbiAgICByZXR1cm4gZmFsc2U7XG4gIH1cbiAgdmFyIHBhdHRlcm4gPSBpc0Z1bmN0aW9uKHZhbHVlKSA/IHJlSXNOYXRpdmUgOiByZUlzSG9zdEN0b3I7XG4gIHJldHVybiBwYXR0ZXJuLnRlc3QodG9Tb3VyY2UodmFsdWUpKTtcbn1cblxubW9kdWxlLmV4cG9ydHMgPSBiYXNlSXNOYXRpdmU7XG4iLCJ2YXIgYmFzZUdldFRhZyA9IHJlcXVpcmUoJy4vX2Jhc2VHZXRUYWcnKSxcbiAgICBpc0xlbmd0aCA9IHJlcXVpcmUoJy4vaXNMZW5ndGgnKSxcbiAgICBpc09iamVjdExpa2UgPSByZXF1aXJlKCcuL2lzT2JqZWN0TGlrZScpO1xuXG4vKiogYE9iamVjdCN0b1N0cmluZ2AgcmVzdWx0IHJlZmVyZW5jZXMuICovXG52YXIgYXJnc1RhZyA9ICdbb2JqZWN0IEFyZ3VtZW50c10nLFxuICAgIGFycmF5VGFnID0gJ1tvYmplY3QgQXJyYXldJyxcbiAgICBib29sVGFnID0gJ1tvYmplY3QgQm9vbGVhbl0nLFxuICAgIGRhdGVUYWcgPSAnW29iamVjdCBEYXRlXScsXG4gICAgZXJyb3JUYWcgPSAnW29iamVjdCBFcnJvcl0nLFxuICAgIGZ1bmNUYWcgPSAnW29iamVjdCBGdW5jdGlvbl0nLFxuICAgIG1hcFRhZyA9ICdbb2JqZWN0IE1hcF0nLFxuICAgIG51bWJlclRhZyA9ICdbb2JqZWN0IE51bWJlcl0nLFxuICAgIG9iamVjdFRhZyA9ICdbb2JqZWN0IE9iamVjdF0nLFxuICAgIHJlZ2V4cFRhZyA9ICdbb2JqZWN0IFJlZ0V4cF0nLFxuICAgIHNldFRhZyA9ICdbb2JqZWN0IFNldF0nLFxuICAgIHN0cmluZ1RhZyA9ICdbb2JqZWN0IFN0cmluZ10nLFxuICAgIHdlYWtNYXBUYWcgPSAnW29iamVjdCBXZWFrTWFwXSc7XG5cbnZhciBhcnJheUJ1ZmZlclRhZyA9ICdbb2JqZWN0IEFycmF5QnVmZmVyXScsXG4gICAgZGF0YVZpZXdUYWcgPSAnW29iamVjdCBEYXRhVmlld10nLFxuICAgIGZsb2F0MzJUYWcgPSAnW29iamVjdCBGbG9hdDMyQXJyYXldJyxcbiAgICBmbG9hdDY0VGFnID0gJ1tvYmplY3QgRmxvYXQ2NEFycmF5XScsXG4gICAgaW50OFRhZyA9ICdbb2JqZWN0IEludDhBcnJheV0nLFxuICAgIGludDE2VGFnID0gJ1tvYmplY3QgSW50MTZBcnJheV0nLFxuICAgIGludDMyVGFnID0gJ1tvYmplY3QgSW50MzJBcnJheV0nLFxuICAgIHVpbnQ4VGFnID0gJ1tvYmplY3QgVWludDhBcnJheV0nLFxuICAgIHVpbnQ4Q2xhbXBlZFRhZyA9ICdbb2JqZWN0IFVpbnQ4Q2xhbXBlZEFycmF5XScsXG4gICAgdWludDE2VGFnID0gJ1tvYmplY3QgVWludDE2QXJyYXldJyxcbiAgICB1aW50MzJUYWcgPSAnW29iamVjdCBVaW50MzJBcnJheV0nO1xuXG4vKiogVXNlZCB0byBpZGVudGlmeSBgdG9TdHJpbmdUYWdgIHZhbHVlcyBvZiB0eXBlZCBhcnJheXMuICovXG52YXIgdHlwZWRBcnJheVRhZ3MgPSB7fTtcbnR5cGVkQXJyYXlUYWdzW2Zsb2F0MzJUYWddID0gdHlwZWRBcnJheVRhZ3NbZmxvYXQ2NFRhZ10gPVxudHlwZWRBcnJheVRhZ3NbaW50OFRhZ10gPSB0eXBlZEFycmF5VGFnc1tpbnQxNlRhZ10gPVxudHlwZWRBcnJheVRhZ3NbaW50MzJUYWddID0gdHlwZWRBcnJheVRhZ3NbdWludDhUYWddID1cbnR5cGVkQXJyYXlUYWdzW3VpbnQ4Q2xhbXBlZFRhZ10gPSB0eXBlZEFycmF5VGFnc1t1aW50MTZUYWddID1cbnR5cGVkQXJyYXlUYWdzW3VpbnQzMlRhZ10gPSB0cnVlO1xudHlwZWRBcnJheVRhZ3NbYXJnc1RhZ10gPSB0eXBlZEFycmF5VGFnc1thcnJheVRhZ10gPVxudHlwZWRBcnJheVRhZ3NbYXJyYXlCdWZmZXJUYWddID0gdHlwZWRBcnJheVRhZ3NbYm9vbFRhZ10gPVxudHlwZWRBcnJheVRhZ3NbZGF0YVZpZXdUYWddID0gdHlwZWRBcnJheVRhZ3NbZGF0ZVRhZ10gPVxudHlwZWRBcnJheVRhZ3NbZXJyb3JUYWddID0gdHlwZWRBcnJheVRhZ3NbZnVuY1RhZ10gPVxudHlwZWRBcnJheVRhZ3NbbWFwVGFnXSA9IHR5cGVkQXJyYXlUYWdzW251bWJlclRhZ10gPVxudHlwZWRBcnJheVRhZ3Nbb2JqZWN0VGFnXSA9IHR5cGVkQXJyYXlUYWdzW3JlZ2V4cFRhZ10gPVxudHlwZWRBcnJheVRhZ3Nbc2V0VGFnXSA9IHR5cGVkQXJyYXlUYWdzW3N0cmluZ1RhZ10gPVxudHlwZWRBcnJheVRhZ3Nbd2Vha01hcFRhZ10gPSBmYWxzZTtcblxuLyoqXG4gKiBUaGUgYmFzZSBpbXBsZW1lbnRhdGlvbiBvZiBgXy5pc1R5cGVkQXJyYXlgIHdpdGhvdXQgTm9kZS5qcyBvcHRpbWl6YXRpb25zLlxuICpcbiAqIEBwcml2YXRlXG4gKiBAcGFyYW0geyp9IHZhbHVlIFRoZSB2YWx1ZSB0byBjaGVjay5cbiAqIEByZXR1cm5zIHtib29sZWFufSBSZXR1cm5zIGB0cnVlYCBpZiBgdmFsdWVgIGlzIGEgdHlwZWQgYXJyYXksIGVsc2UgYGZhbHNlYC5cbiAqL1xuZnVuY3Rpb24gYmFzZUlzVHlwZWRBcnJheSh2YWx1ZSkge1xuICByZXR1cm4gaXNPYmplY3RMaWtlKHZhbHVlKSAmJlxuICAgIGlzTGVuZ3RoKHZhbHVlLmxlbmd0aCkgJiYgISF0eXBlZEFycmF5VGFnc1tiYXNlR2V0VGFnKHZhbHVlKV07XG59XG5cbm1vZHVsZS5leHBvcnRzID0gYmFzZUlzVHlwZWRBcnJheTtcbiIsInZhciBpc1Byb3RvdHlwZSA9IHJlcXVpcmUoJy4vX2lzUHJvdG90eXBlJyksXG4gICAgbmF0aXZlS2V5cyA9IHJlcXVpcmUoJy4vX25hdGl2ZUtleXMnKTtcblxuLyoqIFVzZWQgZm9yIGJ1aWx0LWluIG1ldGhvZCByZWZlcmVuY2VzLiAqL1xudmFyIG9iamVjdFByb3RvID0gT2JqZWN0LnByb3RvdHlwZTtcblxuLyoqIFVzZWQgdG8gY2hlY2sgb2JqZWN0cyBmb3Igb3duIHByb3BlcnRpZXMuICovXG52YXIgaGFzT3duUHJvcGVydHkgPSBvYmplY3RQcm90by5oYXNPd25Qcm9wZXJ0eTtcblxuLyoqXG4gKiBUaGUgYmFzZSBpbXBsZW1lbnRhdGlvbiBvZiBgXy5rZXlzYCB3aGljaCBkb2Vzbid0IHRyZWF0IHNwYXJzZSBhcnJheXMgYXMgZGVuc2UuXG4gKlxuICogQHByaXZhdGVcbiAqIEBwYXJhbSB7T2JqZWN0fSBvYmplY3QgVGhlIG9iamVjdCB0byBxdWVyeS5cbiAqIEByZXR1cm5zIHtBcnJheX0gUmV0dXJucyB0aGUgYXJyYXkgb2YgcHJvcGVydHkgbmFtZXMuXG4gKi9cbmZ1bmN0aW9uIGJhc2VLZXlzKG9iamVjdCkge1xuICBpZiAoIWlzUHJvdG90eXBlKG9iamVjdCkpIHtcbiAgICByZXR1cm4gbmF0aXZlS2V5cyhvYmplY3QpO1xuICB9XG4gIHZhciByZXN1bHQgPSBbXTtcbiAgZm9yICh2YXIga2V5IGluIE9iamVjdChvYmplY3QpKSB7XG4gICAgaWYgKGhhc093blByb3BlcnR5LmNhbGwob2JqZWN0LCBrZXkpICYmIGtleSAhPSAnY29uc3RydWN0b3InKSB7XG4gICAgICByZXN1bHQucHVzaChrZXkpO1xuICAgIH1cbiAgfVxuICByZXR1cm4gcmVzdWx0O1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IGJhc2VLZXlzO1xuIiwiLyoqXG4gKiBUaGUgYmFzZSBpbXBsZW1lbnRhdGlvbiBvZiBgXy50aW1lc2Agd2l0aG91dCBzdXBwb3J0IGZvciBpdGVyYXRlZSBzaG9ydGhhbmRzXG4gKiBvciBtYXggYXJyYXkgbGVuZ3RoIGNoZWNrcy5cbiAqXG4gKiBAcHJpdmF0ZVxuICogQHBhcmFtIHtudW1iZXJ9IG4gVGhlIG51bWJlciBvZiB0aW1lcyB0byBpbnZva2UgYGl0ZXJhdGVlYC5cbiAqIEBwYXJhbSB7RnVuY3Rpb259IGl0ZXJhdGVlIFRoZSBmdW5jdGlvbiBpbnZva2VkIHBlciBpdGVyYXRpb24uXG4gKiBAcmV0dXJucyB7QXJyYXl9IFJldHVybnMgdGhlIGFycmF5IG9mIHJlc3VsdHMuXG4gKi9cbmZ1bmN0aW9uIGJhc2VUaW1lcyhuLCBpdGVyYXRlZSkge1xuICB2YXIgaW5kZXggPSAtMSxcbiAgICAgIHJlc3VsdCA9IEFycmF5KG4pO1xuXG4gIHdoaWxlICgrK2luZGV4IDwgbikge1xuICAgIHJlc3VsdFtpbmRleF0gPSBpdGVyYXRlZShpbmRleCk7XG4gIH1cbiAgcmV0dXJuIHJlc3VsdDtcbn1cblxubW9kdWxlLmV4cG9ydHMgPSBiYXNlVGltZXM7XG4iLCIvKipcbiAqIFRoZSBiYXNlIGltcGxlbWVudGF0aW9uIG9mIGBfLnVuYXJ5YCB3aXRob3V0IHN1cHBvcnQgZm9yIHN0b3JpbmcgbWV0YWRhdGEuXG4gKlxuICogQHByaXZhdGVcbiAqIEBwYXJhbSB7RnVuY3Rpb259IGZ1bmMgVGhlIGZ1bmN0aW9uIHRvIGNhcCBhcmd1bWVudHMgZm9yLlxuICogQHJldHVybnMge0Z1bmN0aW9ufSBSZXR1cm5zIHRoZSBuZXcgY2FwcGVkIGZ1bmN0aW9uLlxuICovXG5mdW5jdGlvbiBiYXNlVW5hcnkoZnVuYykge1xuICByZXR1cm4gZnVuY3Rpb24odmFsdWUpIHtcbiAgICByZXR1cm4gZnVuYyh2YWx1ZSk7XG4gIH07XG59XG5cbm1vZHVsZS5leHBvcnRzID0gYmFzZVVuYXJ5O1xuIiwiLyoqXG4gKiBDaGVja3MgaWYgYSBgY2FjaGVgIHZhbHVlIGZvciBga2V5YCBleGlzdHMuXG4gKlxuICogQHByaXZhdGVcbiAqIEBwYXJhbSB7T2JqZWN0fSBjYWNoZSBUaGUgY2FjaGUgdG8gcXVlcnkuXG4gKiBAcGFyYW0ge3N0cmluZ30ga2V5IFRoZSBrZXkgb2YgdGhlIGVudHJ5IHRvIGNoZWNrLlxuICogQHJldHVybnMge2Jvb2xlYW59IFJldHVybnMgYHRydWVgIGlmIGFuIGVudHJ5IGZvciBga2V5YCBleGlzdHMsIGVsc2UgYGZhbHNlYC5cbiAqL1xuZnVuY3Rpb24gY2FjaGVIYXMoY2FjaGUsIGtleSkge1xuICByZXR1cm4gY2FjaGUuaGFzKGtleSk7XG59XG5cbm1vZHVsZS5leHBvcnRzID0gY2FjaGVIYXM7XG4iLCJ2YXIgcm9vdCA9IHJlcXVpcmUoJy4vX3Jvb3QnKTtcblxuLyoqIFVzZWQgdG8gZGV0ZWN0IG92ZXJyZWFjaGluZyBjb3JlLWpzIHNoaW1zLiAqL1xudmFyIGNvcmVKc0RhdGEgPSByb290WydfX2NvcmUtanNfc2hhcmVkX18nXTtcblxubW9kdWxlLmV4cG9ydHMgPSBjb3JlSnNEYXRhO1xuIiwidmFyIFNldENhY2hlID0gcmVxdWlyZSgnLi9fU2V0Q2FjaGUnKSxcbiAgICBhcnJheVNvbWUgPSByZXF1aXJlKCcuL19hcnJheVNvbWUnKSxcbiAgICBjYWNoZUhhcyA9IHJlcXVpcmUoJy4vX2NhY2hlSGFzJyk7XG5cbi8qKiBVc2VkIHRvIGNvbXBvc2UgYml0bWFza3MgZm9yIHZhbHVlIGNvbXBhcmlzb25zLiAqL1xudmFyIENPTVBBUkVfUEFSVElBTF9GTEFHID0gMSxcbiAgICBDT01QQVJFX1VOT1JERVJFRF9GTEFHID0gMjtcblxuLyoqXG4gKiBBIHNwZWNpYWxpemVkIHZlcnNpb24gb2YgYGJhc2VJc0VxdWFsRGVlcGAgZm9yIGFycmF5cyB3aXRoIHN1cHBvcnQgZm9yXG4gKiBwYXJ0aWFsIGRlZXAgY29tcGFyaXNvbnMuXG4gKlxuICogQHByaXZhdGVcbiAqIEBwYXJhbSB7QXJyYXl9IGFycmF5IFRoZSBhcnJheSB0byBjb21wYXJlLlxuICogQHBhcmFtIHtBcnJheX0gb3RoZXIgVGhlIG90aGVyIGFycmF5IHRvIGNvbXBhcmUuXG4gKiBAcGFyYW0ge251bWJlcn0gYml0bWFzayBUaGUgYml0bWFzayBmbGFncy4gU2VlIGBiYXNlSXNFcXVhbGAgZm9yIG1vcmUgZGV0YWlscy5cbiAqIEBwYXJhbSB7RnVuY3Rpb259IGN1c3RvbWl6ZXIgVGhlIGZ1bmN0aW9uIHRvIGN1c3RvbWl6ZSBjb21wYXJpc29ucy5cbiAqIEBwYXJhbSB7RnVuY3Rpb259IGVxdWFsRnVuYyBUaGUgZnVuY3Rpb24gdG8gZGV0ZXJtaW5lIGVxdWl2YWxlbnRzIG9mIHZhbHVlcy5cbiAqIEBwYXJhbSB7T2JqZWN0fSBzdGFjayBUcmFja3MgdHJhdmVyc2VkIGBhcnJheWAgYW5kIGBvdGhlcmAgb2JqZWN0cy5cbiAqIEByZXR1cm5zIHtib29sZWFufSBSZXR1cm5zIGB0cnVlYCBpZiB0aGUgYXJyYXlzIGFyZSBlcXVpdmFsZW50LCBlbHNlIGBmYWxzZWAuXG4gKi9cbmZ1bmN0aW9uIGVxdWFsQXJyYXlzKGFycmF5LCBvdGhlciwgYml0bWFzaywgY3VzdG9taXplciwgZXF1YWxGdW5jLCBzdGFjaykge1xuICB2YXIgaXNQYXJ0aWFsID0gYml0bWFzayAmIENPTVBBUkVfUEFSVElBTF9GTEFHLFxuICAgICAgYXJyTGVuZ3RoID0gYXJyYXkubGVuZ3RoLFxuICAgICAgb3RoTGVuZ3RoID0gb3RoZXIubGVuZ3RoO1xuXG4gIGlmIChhcnJMZW5ndGggIT0gb3RoTGVuZ3RoICYmICEoaXNQYXJ0aWFsICYmIG90aExlbmd0aCA+IGFyckxlbmd0aCkpIHtcbiAgICByZXR1cm4gZmFsc2U7XG4gIH1cbiAgLy8gQXNzdW1lIGN5Y2xpYyB2YWx1ZXMgYXJlIGVxdWFsLlxuICB2YXIgc3RhY2tlZCA9IHN0YWNrLmdldChhcnJheSk7XG4gIGlmIChzdGFja2VkICYmIHN0YWNrLmdldChvdGhlcikpIHtcbiAgICByZXR1cm4gc3RhY2tlZCA9PSBvdGhlcjtcbiAgfVxuICB2YXIgaW5kZXggPSAtMSxcbiAgICAgIHJlc3VsdCA9IHRydWUsXG4gICAgICBzZWVuID0gKGJpdG1hc2sgJiBDT01QQVJFX1VOT1JERVJFRF9GTEFHKSA/IG5ldyBTZXRDYWNoZSA6IHVuZGVmaW5lZDtcblxuICBzdGFjay5zZXQoYXJyYXksIG90aGVyKTtcbiAgc3RhY2suc2V0KG90aGVyLCBhcnJheSk7XG5cbiAgLy8gSWdub3JlIG5vbi1pbmRleCBwcm9wZXJ0aWVzLlxuICB3aGlsZSAoKytpbmRleCA8IGFyckxlbmd0aCkge1xuICAgIHZhciBhcnJWYWx1ZSA9IGFycmF5W2luZGV4XSxcbiAgICAgICAgb3RoVmFsdWUgPSBvdGhlcltpbmRleF07XG5cbiAgICBpZiAoY3VzdG9taXplcikge1xuICAgICAgdmFyIGNvbXBhcmVkID0gaXNQYXJ0aWFsXG4gICAgICAgID8gY3VzdG9taXplcihvdGhWYWx1ZSwgYXJyVmFsdWUsIGluZGV4LCBvdGhlciwgYXJyYXksIHN0YWNrKVxuICAgICAgICA6IGN1c3RvbWl6ZXIoYXJyVmFsdWUsIG90aFZhbHVlLCBpbmRleCwgYXJyYXksIG90aGVyLCBzdGFjayk7XG4gICAgfVxuICAgIGlmIChjb21wYXJlZCAhPT0gdW5kZWZpbmVkKSB7XG4gICAgICBpZiAoY29tcGFyZWQpIHtcbiAgICAgICAgY29udGludWU7XG4gICAgICB9XG4gICAgICByZXN1bHQgPSBmYWxzZTtcbiAgICAgIGJyZWFrO1xuICAgIH1cbiAgICAvLyBSZWN1cnNpdmVseSBjb21wYXJlIGFycmF5cyAoc3VzY2VwdGlibGUgdG8gY2FsbCBzdGFjayBsaW1pdHMpLlxuICAgIGlmIChzZWVuKSB7XG4gICAgICBpZiAoIWFycmF5U29tZShvdGhlciwgZnVuY3Rpb24ob3RoVmFsdWUsIG90aEluZGV4KSB7XG4gICAgICAgICAgICBpZiAoIWNhY2hlSGFzKHNlZW4sIG90aEluZGV4KSAmJlxuICAgICAgICAgICAgICAgIChhcnJWYWx1ZSA9PT0gb3RoVmFsdWUgfHwgZXF1YWxGdW5jKGFyclZhbHVlLCBvdGhWYWx1ZSwgYml0bWFzaywgY3VzdG9taXplciwgc3RhY2spKSkge1xuICAgICAgICAgICAgICByZXR1cm4gc2Vlbi5wdXNoKG90aEluZGV4KTtcbiAgICAgICAgICAgIH1cbiAgICAgICAgICB9KSkge1xuICAgICAgICByZXN1bHQgPSBmYWxzZTtcbiAgICAgICAgYnJlYWs7XG4gICAgICB9XG4gICAgfSBlbHNlIGlmICghKFxuICAgICAgICAgIGFyclZhbHVlID09PSBvdGhWYWx1ZSB8fFxuICAgICAgICAgICAgZXF1YWxGdW5jKGFyclZhbHVlLCBvdGhWYWx1ZSwgYml0bWFzaywgY3VzdG9taXplciwgc3RhY2spXG4gICAgICAgICkpIHtcbiAgICAgIHJlc3VsdCA9IGZhbHNlO1xuICAgICAgYnJlYWs7XG4gICAgfVxuICB9XG4gIHN0YWNrWydkZWxldGUnXShhcnJheSk7XG4gIHN0YWNrWydkZWxldGUnXShvdGhlcik7XG4gIHJldHVybiByZXN1bHQ7XG59XG5cbm1vZHVsZS5leHBvcnRzID0gZXF1YWxBcnJheXM7XG4iLCJ2YXIgU3ltYm9sID0gcmVxdWlyZSgnLi9fU3ltYm9sJyksXG4gICAgVWludDhBcnJheSA9IHJlcXVpcmUoJy4vX1VpbnQ4QXJyYXknKSxcbiAgICBlcSA9IHJlcXVpcmUoJy4vZXEnKSxcbiAgICBlcXVhbEFycmF5cyA9IHJlcXVpcmUoJy4vX2VxdWFsQXJyYXlzJyksXG4gICAgbWFwVG9BcnJheSA9IHJlcXVpcmUoJy4vX21hcFRvQXJyYXknKSxcbiAgICBzZXRUb0FycmF5ID0gcmVxdWlyZSgnLi9fc2V0VG9BcnJheScpO1xuXG4vKiogVXNlZCB0byBjb21wb3NlIGJpdG1hc2tzIGZvciB2YWx1ZSBjb21wYXJpc29ucy4gKi9cbnZhciBDT01QQVJFX1BBUlRJQUxfRkxBRyA9IDEsXG4gICAgQ09NUEFSRV9VTk9SREVSRURfRkxBRyA9IDI7XG5cbi8qKiBgT2JqZWN0I3RvU3RyaW5nYCByZXN1bHQgcmVmZXJlbmNlcy4gKi9cbnZhciBib29sVGFnID0gJ1tvYmplY3QgQm9vbGVhbl0nLFxuICAgIGRhdGVUYWcgPSAnW29iamVjdCBEYXRlXScsXG4gICAgZXJyb3JUYWcgPSAnW29iamVjdCBFcnJvcl0nLFxuICAgIG1hcFRhZyA9ICdbb2JqZWN0IE1hcF0nLFxuICAgIG51bWJlclRhZyA9ICdbb2JqZWN0IE51bWJlcl0nLFxuICAgIHJlZ2V4cFRhZyA9ICdbb2JqZWN0IFJlZ0V4cF0nLFxuICAgIHNldFRhZyA9ICdbb2JqZWN0IFNldF0nLFxuICAgIHN0cmluZ1RhZyA9ICdbb2JqZWN0IFN0cmluZ10nLFxuICAgIHN5bWJvbFRhZyA9ICdbb2JqZWN0IFN5bWJvbF0nO1xuXG52YXIgYXJyYXlCdWZmZXJUYWcgPSAnW29iamVjdCBBcnJheUJ1ZmZlcl0nLFxuICAgIGRhdGFWaWV3VGFnID0gJ1tvYmplY3QgRGF0YVZpZXddJztcblxuLyoqIFVzZWQgdG8gY29udmVydCBzeW1ib2xzIHRvIHByaW1pdGl2ZXMgYW5kIHN0cmluZ3MuICovXG52YXIgc3ltYm9sUHJvdG8gPSBTeW1ib2wgPyBTeW1ib2wucHJvdG90eXBlIDogdW5kZWZpbmVkLFxuICAgIHN5bWJvbFZhbHVlT2YgPSBzeW1ib2xQcm90byA/IHN5bWJvbFByb3RvLnZhbHVlT2YgOiB1bmRlZmluZWQ7XG5cbi8qKlxuICogQSBzcGVjaWFsaXplZCB2ZXJzaW9uIG9mIGBiYXNlSXNFcXVhbERlZXBgIGZvciBjb21wYXJpbmcgb2JqZWN0cyBvZlxuICogdGhlIHNhbWUgYHRvU3RyaW5nVGFnYC5cbiAqXG4gKiAqKk5vdGU6KiogVGhpcyBmdW5jdGlvbiBvbmx5IHN1cHBvcnRzIGNvbXBhcmluZyB2YWx1ZXMgd2l0aCB0YWdzIG9mXG4gKiBgQm9vbGVhbmAsIGBEYXRlYCwgYEVycm9yYCwgYE51bWJlcmAsIGBSZWdFeHBgLCBvciBgU3RyaW5nYC5cbiAqXG4gKiBAcHJpdmF0ZVxuICogQHBhcmFtIHtPYmplY3R9IG9iamVjdCBUaGUgb2JqZWN0IHRvIGNvbXBhcmUuXG4gKiBAcGFyYW0ge09iamVjdH0gb3RoZXIgVGhlIG90aGVyIG9iamVjdCB0byBjb21wYXJlLlxuICogQHBhcmFtIHtzdHJpbmd9IHRhZyBUaGUgYHRvU3RyaW5nVGFnYCBvZiB0aGUgb2JqZWN0cyB0byBjb21wYXJlLlxuICogQHBhcmFtIHtudW1iZXJ9IGJpdG1hc2sgVGhlIGJpdG1hc2sgZmxhZ3MuIFNlZSBgYmFzZUlzRXF1YWxgIGZvciBtb3JlIGRldGFpbHMuXG4gKiBAcGFyYW0ge0Z1bmN0aW9ufSBjdXN0b21pemVyIFRoZSBmdW5jdGlvbiB0byBjdXN0b21pemUgY29tcGFyaXNvbnMuXG4gKiBAcGFyYW0ge0Z1bmN0aW9ufSBlcXVhbEZ1bmMgVGhlIGZ1bmN0aW9uIHRvIGRldGVybWluZSBlcXVpdmFsZW50cyBvZiB2YWx1ZXMuXG4gKiBAcGFyYW0ge09iamVjdH0gc3RhY2sgVHJhY2tzIHRyYXZlcnNlZCBgb2JqZWN0YCBhbmQgYG90aGVyYCBvYmplY3RzLlxuICogQHJldHVybnMge2Jvb2xlYW59IFJldHVybnMgYHRydWVgIGlmIHRoZSBvYmplY3RzIGFyZSBlcXVpdmFsZW50LCBlbHNlIGBmYWxzZWAuXG4gKi9cbmZ1bmN0aW9uIGVxdWFsQnlUYWcob2JqZWN0LCBvdGhlciwgdGFnLCBiaXRtYXNrLCBjdXN0b21pemVyLCBlcXVhbEZ1bmMsIHN0YWNrKSB7XG4gIHN3aXRjaCAodGFnKSB7XG4gICAgY2FzZSBkYXRhVmlld1RhZzpcbiAgICAgIGlmICgob2JqZWN0LmJ5dGVMZW5ndGggIT0gb3RoZXIuYnl0ZUxlbmd0aCkgfHxcbiAgICAgICAgICAob2JqZWN0LmJ5dGVPZmZzZXQgIT0gb3RoZXIuYnl0ZU9mZnNldCkpIHtcbiAgICAgICAgcmV0dXJuIGZhbHNlO1xuICAgICAgfVxuICAgICAgb2JqZWN0ID0gb2JqZWN0LmJ1ZmZlcjtcbiAgICAgIG90aGVyID0gb3RoZXIuYnVmZmVyO1xuXG4gICAgY2FzZSBhcnJheUJ1ZmZlclRhZzpcbiAgICAgIGlmICgob2JqZWN0LmJ5dGVMZW5ndGggIT0gb3RoZXIuYnl0ZUxlbmd0aCkgfHxcbiAgICAgICAgICAhZXF1YWxGdW5jKG5ldyBVaW50OEFycmF5KG9iamVjdCksIG5ldyBVaW50OEFycmF5KG90aGVyKSkpIHtcbiAgICAgICAgcmV0dXJuIGZhbHNlO1xuICAgICAgfVxuICAgICAgcmV0dXJuIHRydWU7XG5cbiAgICBjYXNlIGJvb2xUYWc6XG4gICAgY2FzZSBkYXRlVGFnOlxuICAgIGNhc2UgbnVtYmVyVGFnOlxuICAgICAgLy8gQ29lcmNlIGJvb2xlYW5zIHRvIGAxYCBvciBgMGAgYW5kIGRhdGVzIHRvIG1pbGxpc2Vjb25kcy5cbiAgICAgIC8vIEludmFsaWQgZGF0ZXMgYXJlIGNvZXJjZWQgdG8gYE5hTmAuXG4gICAgICByZXR1cm4gZXEoK29iamVjdCwgK290aGVyKTtcblxuICAgIGNhc2UgZXJyb3JUYWc6XG4gICAgICByZXR1cm4gb2JqZWN0Lm5hbWUgPT0gb3RoZXIubmFtZSAmJiBvYmplY3QubWVzc2FnZSA9PSBvdGhlci5tZXNzYWdlO1xuXG4gICAgY2FzZSByZWdleHBUYWc6XG4gICAgY2FzZSBzdHJpbmdUYWc6XG4gICAgICAvLyBDb2VyY2UgcmVnZXhlcyB0byBzdHJpbmdzIGFuZCB0cmVhdCBzdHJpbmdzLCBwcmltaXRpdmVzIGFuZCBvYmplY3RzLFxuICAgICAgLy8gYXMgZXF1YWwuIFNlZSBodHRwOi8vd3d3LmVjbWEtaW50ZXJuYXRpb25hbC5vcmcvZWNtYS0yNjIvNy4wLyNzZWMtcmVnZXhwLnByb3RvdHlwZS50b3N0cmluZ1xuICAgICAgLy8gZm9yIG1vcmUgZGV0YWlscy5cbiAgICAgIHJldHVybiBvYmplY3QgPT0gKG90aGVyICsgJycpO1xuXG4gICAgY2FzZSBtYXBUYWc6XG4gICAgICB2YXIgY29udmVydCA9IG1hcFRvQXJyYXk7XG5cbiAgICBjYXNlIHNldFRhZzpcbiAgICAgIHZhciBpc1BhcnRpYWwgPSBiaXRtYXNrICYgQ09NUEFSRV9QQVJUSUFMX0ZMQUc7XG4gICAgICBjb252ZXJ0IHx8IChjb252ZXJ0ID0gc2V0VG9BcnJheSk7XG5cbiAgICAgIGlmIChvYmplY3Quc2l6ZSAhPSBvdGhlci5zaXplICYmICFpc1BhcnRpYWwpIHtcbiAgICAgICAgcmV0dXJuIGZhbHNlO1xuICAgICAgfVxuICAgICAgLy8gQXNzdW1lIGN5Y2xpYyB2YWx1ZXMgYXJlIGVxdWFsLlxuICAgICAgdmFyIHN0YWNrZWQgPSBzdGFjay5nZXQob2JqZWN0KTtcbiAgICAgIGlmIChzdGFja2VkKSB7XG4gICAgICAgIHJldHVybiBzdGFja2VkID09IG90aGVyO1xuICAgICAgfVxuICAgICAgYml0bWFzayB8PSBDT01QQVJFX1VOT1JERVJFRF9GTEFHO1xuXG4gICAgICAvLyBSZWN1cnNpdmVseSBjb21wYXJlIG9iamVjdHMgKHN1c2NlcHRpYmxlIHRvIGNhbGwgc3RhY2sgbGltaXRzKS5cbiAgICAgIHN0YWNrLnNldChvYmplY3QsIG90aGVyKTtcbiAgICAgIHZhciByZXN1bHQgPSBlcXVhbEFycmF5cyhjb252ZXJ0KG9iamVjdCksIGNvbnZlcnQob3RoZXIpLCBiaXRtYXNrLCBjdXN0b21pemVyLCBlcXVhbEZ1bmMsIHN0YWNrKTtcbiAgICAgIHN0YWNrWydkZWxldGUnXShvYmplY3QpO1xuICAgICAgcmV0dXJuIHJlc3VsdDtcblxuICAgIGNhc2Ugc3ltYm9sVGFnOlxuICAgICAgaWYgKHN5bWJvbFZhbHVlT2YpIHtcbiAgICAgICAgcmV0dXJuIHN5bWJvbFZhbHVlT2YuY2FsbChvYmplY3QpID09IHN5bWJvbFZhbHVlT2YuY2FsbChvdGhlcik7XG4gICAgICB9XG4gIH1cbiAgcmV0dXJuIGZhbHNlO1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IGVxdWFsQnlUYWc7XG4iLCJ2YXIgZ2V0QWxsS2V5cyA9IHJlcXVpcmUoJy4vX2dldEFsbEtleXMnKTtcblxuLyoqIFVzZWQgdG8gY29tcG9zZSBiaXRtYXNrcyBmb3IgdmFsdWUgY29tcGFyaXNvbnMuICovXG52YXIgQ09NUEFSRV9QQVJUSUFMX0ZMQUcgPSAxO1xuXG4vKiogVXNlZCBmb3IgYnVpbHQtaW4gbWV0aG9kIHJlZmVyZW5jZXMuICovXG52YXIgb2JqZWN0UHJvdG8gPSBPYmplY3QucHJvdG90eXBlO1xuXG4vKiogVXNlZCB0byBjaGVjayBvYmplY3RzIGZvciBvd24gcHJvcGVydGllcy4gKi9cbnZhciBoYXNPd25Qcm9wZXJ0eSA9IG9iamVjdFByb3RvLmhhc093blByb3BlcnR5O1xuXG4vKipcbiAqIEEgc3BlY2lhbGl6ZWQgdmVyc2lvbiBvZiBgYmFzZUlzRXF1YWxEZWVwYCBmb3Igb2JqZWN0cyB3aXRoIHN1cHBvcnQgZm9yXG4gKiBwYXJ0aWFsIGRlZXAgY29tcGFyaXNvbnMuXG4gKlxuICogQHByaXZhdGVcbiAqIEBwYXJhbSB7T2JqZWN0fSBvYmplY3QgVGhlIG9iamVjdCB0byBjb21wYXJlLlxuICogQHBhcmFtIHtPYmplY3R9IG90aGVyIFRoZSBvdGhlciBvYmplY3QgdG8gY29tcGFyZS5cbiAqIEBwYXJhbSB7bnVtYmVyfSBiaXRtYXNrIFRoZSBiaXRtYXNrIGZsYWdzLiBTZWUgYGJhc2VJc0VxdWFsYCBmb3IgbW9yZSBkZXRhaWxzLlxuICogQHBhcmFtIHtGdW5jdGlvbn0gY3VzdG9taXplciBUaGUgZnVuY3Rpb24gdG8gY3VzdG9taXplIGNvbXBhcmlzb25zLlxuICogQHBhcmFtIHtGdW5jdGlvbn0gZXF1YWxGdW5jIFRoZSBmdW5jdGlvbiB0byBkZXRlcm1pbmUgZXF1aXZhbGVudHMgb2YgdmFsdWVzLlxuICogQHBhcmFtIHtPYmplY3R9IHN0YWNrIFRyYWNrcyB0cmF2ZXJzZWQgYG9iamVjdGAgYW5kIGBvdGhlcmAgb2JqZWN0cy5cbiAqIEByZXR1cm5zIHtib29sZWFufSBSZXR1cm5zIGB0cnVlYCBpZiB0aGUgb2JqZWN0cyBhcmUgZXF1aXZhbGVudCwgZWxzZSBgZmFsc2VgLlxuICovXG5mdW5jdGlvbiBlcXVhbE9iamVjdHMob2JqZWN0LCBvdGhlciwgYml0bWFzaywgY3VzdG9taXplciwgZXF1YWxGdW5jLCBzdGFjaykge1xuICB2YXIgaXNQYXJ0aWFsID0gYml0bWFzayAmIENPTVBBUkVfUEFSVElBTF9GTEFHLFxuICAgICAgb2JqUHJvcHMgPSBnZXRBbGxLZXlzKG9iamVjdCksXG4gICAgICBvYmpMZW5ndGggPSBvYmpQcm9wcy5sZW5ndGgsXG4gICAgICBvdGhQcm9wcyA9IGdldEFsbEtleXMob3RoZXIpLFxuICAgICAgb3RoTGVuZ3RoID0gb3RoUHJvcHMubGVuZ3RoO1xuXG4gIGlmIChvYmpMZW5ndGggIT0gb3RoTGVuZ3RoICYmICFpc1BhcnRpYWwpIHtcbiAgICByZXR1cm4gZmFsc2U7XG4gIH1cbiAgdmFyIGluZGV4ID0gb2JqTGVuZ3RoO1xuICB3aGlsZSAoaW5kZXgtLSkge1xuICAgIHZhciBrZXkgPSBvYmpQcm9wc1tpbmRleF07XG4gICAgaWYgKCEoaXNQYXJ0aWFsID8ga2V5IGluIG90aGVyIDogaGFzT3duUHJvcGVydHkuY2FsbChvdGhlciwga2V5KSkpIHtcbiAgICAgIHJldHVybiBmYWxzZTtcbiAgICB9XG4gIH1cbiAgLy8gQXNzdW1lIGN5Y2xpYyB2YWx1ZXMgYXJlIGVxdWFsLlxuICB2YXIgc3RhY2tlZCA9IHN0YWNrLmdldChvYmplY3QpO1xuICBpZiAoc3RhY2tlZCAmJiBzdGFjay5nZXQob3RoZXIpKSB7XG4gICAgcmV0dXJuIHN0YWNrZWQgPT0gb3RoZXI7XG4gIH1cbiAgdmFyIHJlc3VsdCA9IHRydWU7XG4gIHN0YWNrLnNldChvYmplY3QsIG90aGVyKTtcbiAgc3RhY2suc2V0KG90aGVyLCBvYmplY3QpO1xuXG4gIHZhciBza2lwQ3RvciA9IGlzUGFydGlhbDtcbiAgd2hpbGUgKCsraW5kZXggPCBvYmpMZW5ndGgpIHtcbiAgICBrZXkgPSBvYmpQcm9wc1tpbmRleF07XG4gICAgdmFyIG9ialZhbHVlID0gb2JqZWN0W2tleV0sXG4gICAgICAgIG90aFZhbHVlID0gb3RoZXJba2V5XTtcblxuICAgIGlmIChjdXN0b21pemVyKSB7XG4gICAgICB2YXIgY29tcGFyZWQgPSBpc1BhcnRpYWxcbiAgICAgICAgPyBjdXN0b21pemVyKG90aFZhbHVlLCBvYmpWYWx1ZSwga2V5LCBvdGhlciwgb2JqZWN0LCBzdGFjaylcbiAgICAgICAgOiBjdXN0b21pemVyKG9ialZhbHVlLCBvdGhWYWx1ZSwga2V5LCBvYmplY3QsIG90aGVyLCBzdGFjayk7XG4gICAgfVxuICAgIC8vIFJlY3Vyc2l2ZWx5IGNvbXBhcmUgb2JqZWN0cyAoc3VzY2VwdGlibGUgdG8gY2FsbCBzdGFjayBsaW1pdHMpLlxuICAgIGlmICghKGNvbXBhcmVkID09PSB1bmRlZmluZWRcbiAgICAgICAgICA/IChvYmpWYWx1ZSA9PT0gb3RoVmFsdWUgfHwgZXF1YWxGdW5jKG9ialZhbHVlLCBvdGhWYWx1ZSwgYml0bWFzaywgY3VzdG9taXplciwgc3RhY2spKVxuICAgICAgICAgIDogY29tcGFyZWRcbiAgICAgICAgKSkge1xuICAgICAgcmVzdWx0ID0gZmFsc2U7XG4gICAgICBicmVhaztcbiAgICB9XG4gICAgc2tpcEN0b3IgfHwgKHNraXBDdG9yID0ga2V5ID09ICdjb25zdHJ1Y3RvcicpO1xuICB9XG4gIGlmIChyZXN1bHQgJiYgIXNraXBDdG9yKSB7XG4gICAgdmFyIG9iakN0b3IgPSBvYmplY3QuY29uc3RydWN0b3IsXG4gICAgICAgIG90aEN0b3IgPSBvdGhlci5jb25zdHJ1Y3RvcjtcblxuICAgIC8vIE5vbiBgT2JqZWN0YCBvYmplY3QgaW5zdGFuY2VzIHdpdGggZGlmZmVyZW50IGNvbnN0cnVjdG9ycyBhcmUgbm90IGVxdWFsLlxuICAgIGlmIChvYmpDdG9yICE9IG90aEN0b3IgJiZcbiAgICAgICAgKCdjb25zdHJ1Y3RvcicgaW4gb2JqZWN0ICYmICdjb25zdHJ1Y3RvcicgaW4gb3RoZXIpICYmXG4gICAgICAgICEodHlwZW9mIG9iakN0b3IgPT0gJ2Z1bmN0aW9uJyAmJiBvYmpDdG9yIGluc3RhbmNlb2Ygb2JqQ3RvciAmJlxuICAgICAgICAgIHR5cGVvZiBvdGhDdG9yID09ICdmdW5jdGlvbicgJiYgb3RoQ3RvciBpbnN0YW5jZW9mIG90aEN0b3IpKSB7XG4gICAgICByZXN1bHQgPSBmYWxzZTtcbiAgICB9XG4gIH1cbiAgc3RhY2tbJ2RlbGV0ZSddKG9iamVjdCk7XG4gIHN0YWNrWydkZWxldGUnXShvdGhlcik7XG4gIHJldHVybiByZXN1bHQ7XG59XG5cbm1vZHVsZS5leHBvcnRzID0gZXF1YWxPYmplY3RzO1xuIiwiLyoqIERldGVjdCBmcmVlIHZhcmlhYmxlIGBnbG9iYWxgIGZyb20gTm9kZS5qcy4gKi9cbnZhciBmcmVlR2xvYmFsID0gdHlwZW9mIGdsb2JhbCA9PSAnb2JqZWN0JyAmJiBnbG9iYWwgJiYgZ2xvYmFsLk9iamVjdCA9PT0gT2JqZWN0ICYmIGdsb2JhbDtcblxubW9kdWxlLmV4cG9ydHMgPSBmcmVlR2xvYmFsO1xuIiwidmFyIGJhc2VHZXRBbGxLZXlzID0gcmVxdWlyZSgnLi9fYmFzZUdldEFsbEtleXMnKSxcbiAgICBnZXRTeW1ib2xzID0gcmVxdWlyZSgnLi9fZ2V0U3ltYm9scycpLFxuICAgIGtleXMgPSByZXF1aXJlKCcuL2tleXMnKTtcblxuLyoqXG4gKiBDcmVhdGVzIGFuIGFycmF5IG9mIG93biBlbnVtZXJhYmxlIHByb3BlcnR5IG5hbWVzIGFuZCBzeW1ib2xzIG9mIGBvYmplY3RgLlxuICpcbiAqIEBwcml2YXRlXG4gKiBAcGFyYW0ge09iamVjdH0gb2JqZWN0IFRoZSBvYmplY3QgdG8gcXVlcnkuXG4gKiBAcmV0dXJucyB7QXJyYXl9IFJldHVybnMgdGhlIGFycmF5IG9mIHByb3BlcnR5IG5hbWVzIGFuZCBzeW1ib2xzLlxuICovXG5mdW5jdGlvbiBnZXRBbGxLZXlzKG9iamVjdCkge1xuICByZXR1cm4gYmFzZUdldEFsbEtleXMob2JqZWN0LCBrZXlzLCBnZXRTeW1ib2xzKTtcbn1cblxubW9kdWxlLmV4cG9ydHMgPSBnZXRBbGxLZXlzO1xuIiwidmFyIGlzS2V5YWJsZSA9IHJlcXVpcmUoJy4vX2lzS2V5YWJsZScpO1xuXG4vKipcbiAqIEdldHMgdGhlIGRhdGEgZm9yIGBtYXBgLlxuICpcbiAqIEBwcml2YXRlXG4gKiBAcGFyYW0ge09iamVjdH0gbWFwIFRoZSBtYXAgdG8gcXVlcnkuXG4gKiBAcGFyYW0ge3N0cmluZ30ga2V5IFRoZSByZWZlcmVuY2Uga2V5LlxuICogQHJldHVybnMgeyp9IFJldHVybnMgdGhlIG1hcCBkYXRhLlxuICovXG5mdW5jdGlvbiBnZXRNYXBEYXRhKG1hcCwga2V5KSB7XG4gIHZhciBkYXRhID0gbWFwLl9fZGF0YV9fO1xuICByZXR1cm4gaXNLZXlhYmxlKGtleSlcbiAgICA/IGRhdGFbdHlwZW9mIGtleSA9PSAnc3RyaW5nJyA/ICdzdHJpbmcnIDogJ2hhc2gnXVxuICAgIDogZGF0YS5tYXA7XG59XG5cbm1vZHVsZS5leHBvcnRzID0gZ2V0TWFwRGF0YTtcbiIsInZhciBiYXNlSXNOYXRpdmUgPSByZXF1aXJlKCcuL19iYXNlSXNOYXRpdmUnKSxcbiAgICBnZXRWYWx1ZSA9IHJlcXVpcmUoJy4vX2dldFZhbHVlJyk7XG5cbi8qKlxuICogR2V0cyB0aGUgbmF0aXZlIGZ1bmN0aW9uIGF0IGBrZXlgIG9mIGBvYmplY3RgLlxuICpcbiAqIEBwcml2YXRlXG4gKiBAcGFyYW0ge09iamVjdH0gb2JqZWN0IFRoZSBvYmplY3QgdG8gcXVlcnkuXG4gKiBAcGFyYW0ge3N0cmluZ30ga2V5IFRoZSBrZXkgb2YgdGhlIG1ldGhvZCB0byBnZXQuXG4gKiBAcmV0dXJucyB7Kn0gUmV0dXJucyB0aGUgZnVuY3Rpb24gaWYgaXQncyBuYXRpdmUsIGVsc2UgYHVuZGVmaW5lZGAuXG4gKi9cbmZ1bmN0aW9uIGdldE5hdGl2ZShvYmplY3QsIGtleSkge1xuICB2YXIgdmFsdWUgPSBnZXRWYWx1ZShvYmplY3QsIGtleSk7XG4gIHJldHVybiBiYXNlSXNOYXRpdmUodmFsdWUpID8gdmFsdWUgOiB1bmRlZmluZWQ7XG59XG5cbm1vZHVsZS5leHBvcnRzID0gZ2V0TmF0aXZlO1xuIiwidmFyIFN5bWJvbCA9IHJlcXVpcmUoJy4vX1N5bWJvbCcpO1xuXG4vKiogVXNlZCBmb3IgYnVpbHQtaW4gbWV0aG9kIHJlZmVyZW5jZXMuICovXG52YXIgb2JqZWN0UHJvdG8gPSBPYmplY3QucHJvdG90eXBlO1xuXG4vKiogVXNlZCB0byBjaGVjayBvYmplY3RzIGZvciBvd24gcHJvcGVydGllcy4gKi9cbnZhciBoYXNPd25Qcm9wZXJ0eSA9IG9iamVjdFByb3RvLmhhc093blByb3BlcnR5O1xuXG4vKipcbiAqIFVzZWQgdG8gcmVzb2x2ZSB0aGVcbiAqIFtgdG9TdHJpbmdUYWdgXShodHRwOi8vZWNtYS1pbnRlcm5hdGlvbmFsLm9yZy9lY21hLTI2Mi83LjAvI3NlYy1vYmplY3QucHJvdG90eXBlLnRvc3RyaW5nKVxuICogb2YgdmFsdWVzLlxuICovXG52YXIgbmF0aXZlT2JqZWN0VG9TdHJpbmcgPSBvYmplY3RQcm90by50b1N0cmluZztcblxuLyoqIEJ1aWx0LWluIHZhbHVlIHJlZmVyZW5jZXMuICovXG52YXIgc3ltVG9TdHJpbmdUYWcgPSBTeW1ib2wgPyBTeW1ib2wudG9TdHJpbmdUYWcgOiB1bmRlZmluZWQ7XG5cbi8qKlxuICogQSBzcGVjaWFsaXplZCB2ZXJzaW9uIG9mIGBiYXNlR2V0VGFnYCB3aGljaCBpZ25vcmVzIGBTeW1ib2wudG9TdHJpbmdUYWdgIHZhbHVlcy5cbiAqXG4gKiBAcHJpdmF0ZVxuICogQHBhcmFtIHsqfSB2YWx1ZSBUaGUgdmFsdWUgdG8gcXVlcnkuXG4gKiBAcmV0dXJucyB7c3RyaW5nfSBSZXR1cm5zIHRoZSByYXcgYHRvU3RyaW5nVGFnYC5cbiAqL1xuZnVuY3Rpb24gZ2V0UmF3VGFnKHZhbHVlKSB7XG4gIHZhciBpc093biA9IGhhc093blByb3BlcnR5LmNhbGwodmFsdWUsIHN5bVRvU3RyaW5nVGFnKSxcbiAgICAgIHRhZyA9IHZhbHVlW3N5bVRvU3RyaW5nVGFnXTtcblxuICB0cnkge1xuICAgIHZhbHVlW3N5bVRvU3RyaW5nVGFnXSA9IHVuZGVmaW5lZDtcbiAgICB2YXIgdW5tYXNrZWQgPSB0cnVlO1xuICB9IGNhdGNoIChlKSB7fVxuXG4gIHZhciByZXN1bHQgPSBuYXRpdmVPYmplY3RUb1N0cmluZy5jYWxsKHZhbHVlKTtcbiAgaWYgKHVubWFza2VkKSB7XG4gICAgaWYgKGlzT3duKSB7XG4gICAgICB2YWx1ZVtzeW1Ub1N0cmluZ1RhZ10gPSB0YWc7XG4gICAgfSBlbHNlIHtcbiAgICAgIGRlbGV0ZSB2YWx1ZVtzeW1Ub1N0cmluZ1RhZ107XG4gICAgfVxuICB9XG4gIHJldHVybiByZXN1bHQ7XG59XG5cbm1vZHVsZS5leHBvcnRzID0gZ2V0UmF3VGFnO1xuIiwidmFyIGFycmF5RmlsdGVyID0gcmVxdWlyZSgnLi9fYXJyYXlGaWx0ZXInKSxcbiAgICBzdHViQXJyYXkgPSByZXF1aXJlKCcuL3N0dWJBcnJheScpO1xuXG4vKiogVXNlZCBmb3IgYnVpbHQtaW4gbWV0aG9kIHJlZmVyZW5jZXMuICovXG52YXIgb2JqZWN0UHJvdG8gPSBPYmplY3QucHJvdG90eXBlO1xuXG4vKiogQnVpbHQtaW4gdmFsdWUgcmVmZXJlbmNlcy4gKi9cbnZhciBwcm9wZXJ0eUlzRW51bWVyYWJsZSA9IG9iamVjdFByb3RvLnByb3BlcnR5SXNFbnVtZXJhYmxlO1xuXG4vKiBCdWlsdC1pbiBtZXRob2QgcmVmZXJlbmNlcyBmb3IgdGhvc2Ugd2l0aCB0aGUgc2FtZSBuYW1lIGFzIG90aGVyIGBsb2Rhc2hgIG1ldGhvZHMuICovXG52YXIgbmF0aXZlR2V0U3ltYm9scyA9IE9iamVjdC5nZXRPd25Qcm9wZXJ0eVN5bWJvbHM7XG5cbi8qKlxuICogQ3JlYXRlcyBhbiBhcnJheSBvZiB0aGUgb3duIGVudW1lcmFibGUgc3ltYm9scyBvZiBgb2JqZWN0YC5cbiAqXG4gKiBAcHJpdmF0ZVxuICogQHBhcmFtIHtPYmplY3R9IG9iamVjdCBUaGUgb2JqZWN0IHRvIHF1ZXJ5LlxuICogQHJldHVybnMge0FycmF5fSBSZXR1cm5zIHRoZSBhcnJheSBvZiBzeW1ib2xzLlxuICovXG52YXIgZ2V0U3ltYm9scyA9ICFuYXRpdmVHZXRTeW1ib2xzID8gc3R1YkFycmF5IDogZnVuY3Rpb24ob2JqZWN0KSB7XG4gIGlmIChvYmplY3QgPT0gbnVsbCkge1xuICAgIHJldHVybiBbXTtcbiAgfVxuICBvYmplY3QgPSBPYmplY3Qob2JqZWN0KTtcbiAgcmV0dXJuIGFycmF5RmlsdGVyKG5hdGl2ZUdldFN5bWJvbHMob2JqZWN0KSwgZnVuY3Rpb24oc3ltYm9sKSB7XG4gICAgcmV0dXJuIHByb3BlcnR5SXNFbnVtZXJhYmxlLmNhbGwob2JqZWN0LCBzeW1ib2wpO1xuICB9KTtcbn07XG5cbm1vZHVsZS5leHBvcnRzID0gZ2V0U3ltYm9scztcbiIsInZhciBEYXRhVmlldyA9IHJlcXVpcmUoJy4vX0RhdGFWaWV3JyksXG4gICAgTWFwID0gcmVxdWlyZSgnLi9fTWFwJyksXG4gICAgUHJvbWlzZSA9IHJlcXVpcmUoJy4vX1Byb21pc2UnKSxcbiAgICBTZXQgPSByZXF1aXJlKCcuL19TZXQnKSxcbiAgICBXZWFrTWFwID0gcmVxdWlyZSgnLi9fV2Vha01hcCcpLFxuICAgIGJhc2VHZXRUYWcgPSByZXF1aXJlKCcuL19iYXNlR2V0VGFnJyksXG4gICAgdG9Tb3VyY2UgPSByZXF1aXJlKCcuL190b1NvdXJjZScpO1xuXG4vKiogYE9iamVjdCN0b1N0cmluZ2AgcmVzdWx0IHJlZmVyZW5jZXMuICovXG52YXIgbWFwVGFnID0gJ1tvYmplY3QgTWFwXScsXG4gICAgb2JqZWN0VGFnID0gJ1tvYmplY3QgT2JqZWN0XScsXG4gICAgcHJvbWlzZVRhZyA9ICdbb2JqZWN0IFByb21pc2VdJyxcbiAgICBzZXRUYWcgPSAnW29iamVjdCBTZXRdJyxcbiAgICB3ZWFrTWFwVGFnID0gJ1tvYmplY3QgV2Vha01hcF0nO1xuXG52YXIgZGF0YVZpZXdUYWcgPSAnW29iamVjdCBEYXRhVmlld10nO1xuXG4vKiogVXNlZCB0byBkZXRlY3QgbWFwcywgc2V0cywgYW5kIHdlYWttYXBzLiAqL1xudmFyIGRhdGFWaWV3Q3RvclN0cmluZyA9IHRvU291cmNlKERhdGFWaWV3KSxcbiAgICBtYXBDdG9yU3RyaW5nID0gdG9Tb3VyY2UoTWFwKSxcbiAgICBwcm9taXNlQ3RvclN0cmluZyA9IHRvU291cmNlKFByb21pc2UpLFxuICAgIHNldEN0b3JTdHJpbmcgPSB0b1NvdXJjZShTZXQpLFxuICAgIHdlYWtNYXBDdG9yU3RyaW5nID0gdG9Tb3VyY2UoV2Vha01hcCk7XG5cbi8qKlxuICogR2V0cyB0aGUgYHRvU3RyaW5nVGFnYCBvZiBgdmFsdWVgLlxuICpcbiAqIEBwcml2YXRlXG4gKiBAcGFyYW0geyp9IHZhbHVlIFRoZSB2YWx1ZSB0byBxdWVyeS5cbiAqIEByZXR1cm5zIHtzdHJpbmd9IFJldHVybnMgdGhlIGB0b1N0cmluZ1RhZ2AuXG4gKi9cbnZhciBnZXRUYWcgPSBiYXNlR2V0VGFnO1xuXG4vLyBGYWxsYmFjayBmb3IgZGF0YSB2aWV3cywgbWFwcywgc2V0cywgYW5kIHdlYWsgbWFwcyBpbiBJRSAxMSBhbmQgcHJvbWlzZXMgaW4gTm9kZS5qcyA8IDYuXG5pZiAoKERhdGFWaWV3ICYmIGdldFRhZyhuZXcgRGF0YVZpZXcobmV3IEFycmF5QnVmZmVyKDEpKSkgIT0gZGF0YVZpZXdUYWcpIHx8XG4gICAgKE1hcCAmJiBnZXRUYWcobmV3IE1hcCkgIT0gbWFwVGFnKSB8fFxuICAgIChQcm9taXNlICYmIGdldFRhZyhQcm9taXNlLnJlc29sdmUoKSkgIT0gcHJvbWlzZVRhZykgfHxcbiAgICAoU2V0ICYmIGdldFRhZyhuZXcgU2V0KSAhPSBzZXRUYWcpIHx8XG4gICAgKFdlYWtNYXAgJiYgZ2V0VGFnKG5ldyBXZWFrTWFwKSAhPSB3ZWFrTWFwVGFnKSkge1xuICBnZXRUYWcgPSBmdW5jdGlvbih2YWx1ZSkge1xuICAgIHZhciByZXN1bHQgPSBiYXNlR2V0VGFnKHZhbHVlKSxcbiAgICAgICAgQ3RvciA9IHJlc3VsdCA9PSBvYmplY3RUYWcgPyB2YWx1ZS5jb25zdHJ1Y3RvciA6IHVuZGVmaW5lZCxcbiAgICAgICAgY3RvclN0cmluZyA9IEN0b3IgPyB0b1NvdXJjZShDdG9yKSA6ICcnO1xuXG4gICAgaWYgKGN0b3JTdHJpbmcpIHtcbiAgICAgIHN3aXRjaCAoY3RvclN0cmluZykge1xuICAgICAgICBjYXNlIGRhdGFWaWV3Q3RvclN0cmluZzogcmV0dXJuIGRhdGFWaWV3VGFnO1xuICAgICAgICBjYXNlIG1hcEN0b3JTdHJpbmc6IHJldHVybiBtYXBUYWc7XG4gICAgICAgIGNhc2UgcHJvbWlzZUN0b3JTdHJpbmc6IHJldHVybiBwcm9taXNlVGFnO1xuICAgICAgICBjYXNlIHNldEN0b3JTdHJpbmc6IHJldHVybiBzZXRUYWc7XG4gICAgICAgIGNhc2Ugd2Vha01hcEN0b3JTdHJpbmc6IHJldHVybiB3ZWFrTWFwVGFnO1xuICAgICAgfVxuICAgIH1cbiAgICByZXR1cm4gcmVzdWx0O1xuICB9O1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IGdldFRhZztcbiIsIi8qKlxuICogR2V0cyB0aGUgdmFsdWUgYXQgYGtleWAgb2YgYG9iamVjdGAuXG4gKlxuICogQHByaXZhdGVcbiAqIEBwYXJhbSB7T2JqZWN0fSBbb2JqZWN0XSBUaGUgb2JqZWN0IHRvIHF1ZXJ5LlxuICogQHBhcmFtIHtzdHJpbmd9IGtleSBUaGUga2V5IG9mIHRoZSBwcm9wZXJ0eSB0byBnZXQuXG4gKiBAcmV0dXJucyB7Kn0gUmV0dXJucyB0aGUgcHJvcGVydHkgdmFsdWUuXG4gKi9cbmZ1bmN0aW9uIGdldFZhbHVlKG9iamVjdCwga2V5KSB7XG4gIHJldHVybiBvYmplY3QgPT0gbnVsbCA/IHVuZGVmaW5lZCA6IG9iamVjdFtrZXldO1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IGdldFZhbHVlO1xuIiwidmFyIG5hdGl2ZUNyZWF0ZSA9IHJlcXVpcmUoJy4vX25hdGl2ZUNyZWF0ZScpO1xuXG4vKipcbiAqIFJlbW92ZXMgYWxsIGtleS12YWx1ZSBlbnRyaWVzIGZyb20gdGhlIGhhc2guXG4gKlxuICogQHByaXZhdGVcbiAqIEBuYW1lIGNsZWFyXG4gKiBAbWVtYmVyT2YgSGFzaFxuICovXG5mdW5jdGlvbiBoYXNoQ2xlYXIoKSB7XG4gIHRoaXMuX19kYXRhX18gPSBuYXRpdmVDcmVhdGUgPyBuYXRpdmVDcmVhdGUobnVsbCkgOiB7fTtcbiAgdGhpcy5zaXplID0gMDtcbn1cblxubW9kdWxlLmV4cG9ydHMgPSBoYXNoQ2xlYXI7XG4iLCIvKipcbiAqIFJlbW92ZXMgYGtleWAgYW5kIGl0cyB2YWx1ZSBmcm9tIHRoZSBoYXNoLlxuICpcbiAqIEBwcml2YXRlXG4gKiBAbmFtZSBkZWxldGVcbiAqIEBtZW1iZXJPZiBIYXNoXG4gKiBAcGFyYW0ge09iamVjdH0gaGFzaCBUaGUgaGFzaCB0byBtb2RpZnkuXG4gKiBAcGFyYW0ge3N0cmluZ30ga2V5IFRoZSBrZXkgb2YgdGhlIHZhbHVlIHRvIHJlbW92ZS5cbiAqIEByZXR1cm5zIHtib29sZWFufSBSZXR1cm5zIGB0cnVlYCBpZiB0aGUgZW50cnkgd2FzIHJlbW92ZWQsIGVsc2UgYGZhbHNlYC5cbiAqL1xuZnVuY3Rpb24gaGFzaERlbGV0ZShrZXkpIHtcbiAgdmFyIHJlc3VsdCA9IHRoaXMuaGFzKGtleSkgJiYgZGVsZXRlIHRoaXMuX19kYXRhX19ba2V5XTtcbiAgdGhpcy5zaXplIC09IHJlc3VsdCA/IDEgOiAwO1xuICByZXR1cm4gcmVzdWx0O1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IGhhc2hEZWxldGU7XG4iLCJ2YXIgbmF0aXZlQ3JlYXRlID0gcmVxdWlyZSgnLi9fbmF0aXZlQ3JlYXRlJyk7XG5cbi8qKiBVc2VkIHRvIHN0YW5kLWluIGZvciBgdW5kZWZpbmVkYCBoYXNoIHZhbHVlcy4gKi9cbnZhciBIQVNIX1VOREVGSU5FRCA9ICdfX2xvZGFzaF9oYXNoX3VuZGVmaW5lZF9fJztcblxuLyoqIFVzZWQgZm9yIGJ1aWx0LWluIG1ldGhvZCByZWZlcmVuY2VzLiAqL1xudmFyIG9iamVjdFByb3RvID0gT2JqZWN0LnByb3RvdHlwZTtcblxuLyoqIFVzZWQgdG8gY2hlY2sgb2JqZWN0cyBmb3Igb3duIHByb3BlcnRpZXMuICovXG52YXIgaGFzT3duUHJvcGVydHkgPSBvYmplY3RQcm90by5oYXNPd25Qcm9wZXJ0eTtcblxuLyoqXG4gKiBHZXRzIHRoZSBoYXNoIHZhbHVlIGZvciBga2V5YC5cbiAqXG4gKiBAcHJpdmF0ZVxuICogQG5hbWUgZ2V0XG4gKiBAbWVtYmVyT2YgSGFzaFxuICogQHBhcmFtIHtzdHJpbmd9IGtleSBUaGUga2V5IG9mIHRoZSB2YWx1ZSB0byBnZXQuXG4gKiBAcmV0dXJucyB7Kn0gUmV0dXJucyB0aGUgZW50cnkgdmFsdWUuXG4gKi9cbmZ1bmN0aW9uIGhhc2hHZXQoa2V5KSB7XG4gIHZhciBkYXRhID0gdGhpcy5fX2RhdGFfXztcbiAgaWYgKG5hdGl2ZUNyZWF0ZSkge1xuICAgIHZhciByZXN1bHQgPSBkYXRhW2tleV07XG4gICAgcmV0dXJuIHJlc3VsdCA9PT0gSEFTSF9VTkRFRklORUQgPyB1bmRlZmluZWQgOiByZXN1bHQ7XG4gIH1cbiAgcmV0dXJuIGhhc093blByb3BlcnR5LmNhbGwoZGF0YSwga2V5KSA/IGRhdGFba2V5XSA6IHVuZGVmaW5lZDtcbn1cblxubW9kdWxlLmV4cG9ydHMgPSBoYXNoR2V0O1xuIiwidmFyIG5hdGl2ZUNyZWF0ZSA9IHJlcXVpcmUoJy4vX25hdGl2ZUNyZWF0ZScpO1xuXG4vKiogVXNlZCBmb3IgYnVpbHQtaW4gbWV0aG9kIHJlZmVyZW5jZXMuICovXG52YXIgb2JqZWN0UHJvdG8gPSBPYmplY3QucHJvdG90eXBlO1xuXG4vKiogVXNlZCB0byBjaGVjayBvYmplY3RzIGZvciBvd24gcHJvcGVydGllcy4gKi9cbnZhciBoYXNPd25Qcm9wZXJ0eSA9IG9iamVjdFByb3RvLmhhc093blByb3BlcnR5O1xuXG4vKipcbiAqIENoZWNrcyBpZiBhIGhhc2ggdmFsdWUgZm9yIGBrZXlgIGV4aXN0cy5cbiAqXG4gKiBAcHJpdmF0ZVxuICogQG5hbWUgaGFzXG4gKiBAbWVtYmVyT2YgSGFzaFxuICogQHBhcmFtIHtzdHJpbmd9IGtleSBUaGUga2V5IG9mIHRoZSBlbnRyeSB0byBjaGVjay5cbiAqIEByZXR1cm5zIHtib29sZWFufSBSZXR1cm5zIGB0cnVlYCBpZiBhbiBlbnRyeSBmb3IgYGtleWAgZXhpc3RzLCBlbHNlIGBmYWxzZWAuXG4gKi9cbmZ1bmN0aW9uIGhhc2hIYXMoa2V5KSB7XG4gIHZhciBkYXRhID0gdGhpcy5fX2RhdGFfXztcbiAgcmV0dXJuIG5hdGl2ZUNyZWF0ZSA/IChkYXRhW2tleV0gIT09IHVuZGVmaW5lZCkgOiBoYXNPd25Qcm9wZXJ0eS5jYWxsKGRhdGEsIGtleSk7XG59XG5cbm1vZHVsZS5leHBvcnRzID0gaGFzaEhhcztcbiIsInZhciBuYXRpdmVDcmVhdGUgPSByZXF1aXJlKCcuL19uYXRpdmVDcmVhdGUnKTtcblxuLyoqIFVzZWQgdG8gc3RhbmQtaW4gZm9yIGB1bmRlZmluZWRgIGhhc2ggdmFsdWVzLiAqL1xudmFyIEhBU0hfVU5ERUZJTkVEID0gJ19fbG9kYXNoX2hhc2hfdW5kZWZpbmVkX18nO1xuXG4vKipcbiAqIFNldHMgdGhlIGhhc2ggYGtleWAgdG8gYHZhbHVlYC5cbiAqXG4gKiBAcHJpdmF0ZVxuICogQG5hbWUgc2V0XG4gKiBAbWVtYmVyT2YgSGFzaFxuICogQHBhcmFtIHtzdHJpbmd9IGtleSBUaGUga2V5IG9mIHRoZSB2YWx1ZSB0byBzZXQuXG4gKiBAcGFyYW0geyp9IHZhbHVlIFRoZSB2YWx1ZSB0byBzZXQuXG4gKiBAcmV0dXJucyB7T2JqZWN0fSBSZXR1cm5zIHRoZSBoYXNoIGluc3RhbmNlLlxuICovXG5mdW5jdGlvbiBoYXNoU2V0KGtleSwgdmFsdWUpIHtcbiAgdmFyIGRhdGEgPSB0aGlzLl9fZGF0YV9fO1xuICB0aGlzLnNpemUgKz0gdGhpcy5oYXMoa2V5KSA/IDAgOiAxO1xuICBkYXRhW2tleV0gPSAobmF0aXZlQ3JlYXRlICYmIHZhbHVlID09PSB1bmRlZmluZWQpID8gSEFTSF9VTkRFRklORUQgOiB2YWx1ZTtcbiAgcmV0dXJuIHRoaXM7XG59XG5cbm1vZHVsZS5leHBvcnRzID0gaGFzaFNldDtcbiIsIi8qKiBVc2VkIGFzIHJlZmVyZW5jZXMgZm9yIHZhcmlvdXMgYE51bWJlcmAgY29uc3RhbnRzLiAqL1xudmFyIE1BWF9TQUZFX0lOVEVHRVIgPSA5MDA3MTk5MjU0NzQwOTkxO1xuXG4vKiogVXNlZCB0byBkZXRlY3QgdW5zaWduZWQgaW50ZWdlciB2YWx1ZXMuICovXG52YXIgcmVJc1VpbnQgPSAvXig/OjB8WzEtOV1cXGQqKSQvO1xuXG4vKipcbiAqIENoZWNrcyBpZiBgdmFsdWVgIGlzIGEgdmFsaWQgYXJyYXktbGlrZSBpbmRleC5cbiAqXG4gKiBAcHJpdmF0ZVxuICogQHBhcmFtIHsqfSB2YWx1ZSBUaGUgdmFsdWUgdG8gY2hlY2suXG4gKiBAcGFyYW0ge251bWJlcn0gW2xlbmd0aD1NQVhfU0FGRV9JTlRFR0VSXSBUaGUgdXBwZXIgYm91bmRzIG9mIGEgdmFsaWQgaW5kZXguXG4gKiBAcmV0dXJucyB7Ym9vbGVhbn0gUmV0dXJucyBgdHJ1ZWAgaWYgYHZhbHVlYCBpcyBhIHZhbGlkIGluZGV4LCBlbHNlIGBmYWxzZWAuXG4gKi9cbmZ1bmN0aW9uIGlzSW5kZXgodmFsdWUsIGxlbmd0aCkge1xuICB2YXIgdHlwZSA9IHR5cGVvZiB2YWx1ZTtcbiAgbGVuZ3RoID0gbGVuZ3RoID09IG51bGwgPyBNQVhfU0FGRV9JTlRFR0VSIDogbGVuZ3RoO1xuXG4gIHJldHVybiAhIWxlbmd0aCAmJlxuICAgICh0eXBlID09ICdudW1iZXInIHx8XG4gICAgICAodHlwZSAhPSAnc3ltYm9sJyAmJiByZUlzVWludC50ZXN0KHZhbHVlKSkpICYmXG4gICAgICAgICh2YWx1ZSA+IC0xICYmIHZhbHVlICUgMSA9PSAwICYmIHZhbHVlIDwgbGVuZ3RoKTtcbn1cblxubW9kdWxlLmV4cG9ydHMgPSBpc0luZGV4O1xuIiwiLyoqXG4gKiBDaGVja3MgaWYgYHZhbHVlYCBpcyBzdWl0YWJsZSBmb3IgdXNlIGFzIHVuaXF1ZSBvYmplY3Qga2V5LlxuICpcbiAqIEBwcml2YXRlXG4gKiBAcGFyYW0geyp9IHZhbHVlIFRoZSB2YWx1ZSB0byBjaGVjay5cbiAqIEByZXR1cm5zIHtib29sZWFufSBSZXR1cm5zIGB0cnVlYCBpZiBgdmFsdWVgIGlzIHN1aXRhYmxlLCBlbHNlIGBmYWxzZWAuXG4gKi9cbmZ1bmN0aW9uIGlzS2V5YWJsZSh2YWx1ZSkge1xuICB2YXIgdHlwZSA9IHR5cGVvZiB2YWx1ZTtcbiAgcmV0dXJuICh0eXBlID09ICdzdHJpbmcnIHx8IHR5cGUgPT0gJ251bWJlcicgfHwgdHlwZSA9PSAnc3ltYm9sJyB8fCB0eXBlID09ICdib29sZWFuJylcbiAgICA/ICh2YWx1ZSAhPT0gJ19fcHJvdG9fXycpXG4gICAgOiAodmFsdWUgPT09IG51bGwpO1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IGlzS2V5YWJsZTtcbiIsInZhciBjb3JlSnNEYXRhID0gcmVxdWlyZSgnLi9fY29yZUpzRGF0YScpO1xuXG4vKiogVXNlZCB0byBkZXRlY3QgbWV0aG9kcyBtYXNxdWVyYWRpbmcgYXMgbmF0aXZlLiAqL1xudmFyIG1hc2tTcmNLZXkgPSAoZnVuY3Rpb24oKSB7XG4gIHZhciB1aWQgPSAvW14uXSskLy5leGVjKGNvcmVKc0RhdGEgJiYgY29yZUpzRGF0YS5rZXlzICYmIGNvcmVKc0RhdGEua2V5cy5JRV9QUk9UTyB8fCAnJyk7XG4gIHJldHVybiB1aWQgPyAoJ1N5bWJvbChzcmMpXzEuJyArIHVpZCkgOiAnJztcbn0oKSk7XG5cbi8qKlxuICogQ2hlY2tzIGlmIGBmdW5jYCBoYXMgaXRzIHNvdXJjZSBtYXNrZWQuXG4gKlxuICogQHByaXZhdGVcbiAqIEBwYXJhbSB7RnVuY3Rpb259IGZ1bmMgVGhlIGZ1bmN0aW9uIHRvIGNoZWNrLlxuICogQHJldHVybnMge2Jvb2xlYW59IFJldHVybnMgYHRydWVgIGlmIGBmdW5jYCBpcyBtYXNrZWQsIGVsc2UgYGZhbHNlYC5cbiAqL1xuZnVuY3Rpb24gaXNNYXNrZWQoZnVuYykge1xuICByZXR1cm4gISFtYXNrU3JjS2V5ICYmIChtYXNrU3JjS2V5IGluIGZ1bmMpO1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IGlzTWFza2VkO1xuIiwiLyoqIFVzZWQgZm9yIGJ1aWx0LWluIG1ldGhvZCByZWZlcmVuY2VzLiAqL1xudmFyIG9iamVjdFByb3RvID0gT2JqZWN0LnByb3RvdHlwZTtcblxuLyoqXG4gKiBDaGVja3MgaWYgYHZhbHVlYCBpcyBsaWtlbHkgYSBwcm90b3R5cGUgb2JqZWN0LlxuICpcbiAqIEBwcml2YXRlXG4gKiBAcGFyYW0geyp9IHZhbHVlIFRoZSB2YWx1ZSB0byBjaGVjay5cbiAqIEByZXR1cm5zIHtib29sZWFufSBSZXR1cm5zIGB0cnVlYCBpZiBgdmFsdWVgIGlzIGEgcHJvdG90eXBlLCBlbHNlIGBmYWxzZWAuXG4gKi9cbmZ1bmN0aW9uIGlzUHJvdG90eXBlKHZhbHVlKSB7XG4gIHZhciBDdG9yID0gdmFsdWUgJiYgdmFsdWUuY29uc3RydWN0b3IsXG4gICAgICBwcm90byA9ICh0eXBlb2YgQ3RvciA9PSAnZnVuY3Rpb24nICYmIEN0b3IucHJvdG90eXBlKSB8fCBvYmplY3RQcm90bztcblxuICByZXR1cm4gdmFsdWUgPT09IHByb3RvO1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IGlzUHJvdG90eXBlO1xuIiwiLyoqXG4gKiBSZW1vdmVzIGFsbCBrZXktdmFsdWUgZW50cmllcyBmcm9tIHRoZSBsaXN0IGNhY2hlLlxuICpcbiAqIEBwcml2YXRlXG4gKiBAbmFtZSBjbGVhclxuICogQG1lbWJlck9mIExpc3RDYWNoZVxuICovXG5mdW5jdGlvbiBsaXN0Q2FjaGVDbGVhcigpIHtcbiAgdGhpcy5fX2RhdGFfXyA9IFtdO1xuICB0aGlzLnNpemUgPSAwO1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IGxpc3RDYWNoZUNsZWFyO1xuIiwidmFyIGFzc29jSW5kZXhPZiA9IHJlcXVpcmUoJy4vX2Fzc29jSW5kZXhPZicpO1xuXG4vKiogVXNlZCBmb3IgYnVpbHQtaW4gbWV0aG9kIHJlZmVyZW5jZXMuICovXG52YXIgYXJyYXlQcm90byA9IEFycmF5LnByb3RvdHlwZTtcblxuLyoqIEJ1aWx0LWluIHZhbHVlIHJlZmVyZW5jZXMuICovXG52YXIgc3BsaWNlID0gYXJyYXlQcm90by5zcGxpY2U7XG5cbi8qKlxuICogUmVtb3ZlcyBga2V5YCBhbmQgaXRzIHZhbHVlIGZyb20gdGhlIGxpc3QgY2FjaGUuXG4gKlxuICogQHByaXZhdGVcbiAqIEBuYW1lIGRlbGV0ZVxuICogQG1lbWJlck9mIExpc3RDYWNoZVxuICogQHBhcmFtIHtzdHJpbmd9IGtleSBUaGUga2V5IG9mIHRoZSB2YWx1ZSB0byByZW1vdmUuXG4gKiBAcmV0dXJucyB7Ym9vbGVhbn0gUmV0dXJucyBgdHJ1ZWAgaWYgdGhlIGVudHJ5IHdhcyByZW1vdmVkLCBlbHNlIGBmYWxzZWAuXG4gKi9cbmZ1bmN0aW9uIGxpc3RDYWNoZURlbGV0ZShrZXkpIHtcbiAgdmFyIGRhdGEgPSB0aGlzLl9fZGF0YV9fLFxuICAgICAgaW5kZXggPSBhc3NvY0luZGV4T2YoZGF0YSwga2V5KTtcblxuICBpZiAoaW5kZXggPCAwKSB7XG4gICAgcmV0dXJuIGZhbHNlO1xuICB9XG4gIHZhciBsYXN0SW5kZXggPSBkYXRhLmxlbmd0aCAtIDE7XG4gIGlmIChpbmRleCA9PSBsYXN0SW5kZXgpIHtcbiAgICBkYXRhLnBvcCgpO1xuICB9IGVsc2Uge1xuICAgIHNwbGljZS5jYWxsKGRhdGEsIGluZGV4LCAxKTtcbiAgfVxuICAtLXRoaXMuc2l6ZTtcbiAgcmV0dXJuIHRydWU7XG59XG5cbm1vZHVsZS5leHBvcnRzID0gbGlzdENhY2hlRGVsZXRlO1xuIiwidmFyIGFzc29jSW5kZXhPZiA9IHJlcXVpcmUoJy4vX2Fzc29jSW5kZXhPZicpO1xuXG4vKipcbiAqIEdldHMgdGhlIGxpc3QgY2FjaGUgdmFsdWUgZm9yIGBrZXlgLlxuICpcbiAqIEBwcml2YXRlXG4gKiBAbmFtZSBnZXRcbiAqIEBtZW1iZXJPZiBMaXN0Q2FjaGVcbiAqIEBwYXJhbSB7c3RyaW5nfSBrZXkgVGhlIGtleSBvZiB0aGUgdmFsdWUgdG8gZ2V0LlxuICogQHJldHVybnMgeyp9IFJldHVybnMgdGhlIGVudHJ5IHZhbHVlLlxuICovXG5mdW5jdGlvbiBsaXN0Q2FjaGVHZXQoa2V5KSB7XG4gIHZhciBkYXRhID0gdGhpcy5fX2RhdGFfXyxcbiAgICAgIGluZGV4ID0gYXNzb2NJbmRleE9mKGRhdGEsIGtleSk7XG5cbiAgcmV0dXJuIGluZGV4IDwgMCA/IHVuZGVmaW5lZCA6IGRhdGFbaW5kZXhdWzFdO1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IGxpc3RDYWNoZUdldDtcbiIsInZhciBhc3NvY0luZGV4T2YgPSByZXF1aXJlKCcuL19hc3NvY0luZGV4T2YnKTtcblxuLyoqXG4gKiBDaGVja3MgaWYgYSBsaXN0IGNhY2hlIHZhbHVlIGZvciBga2V5YCBleGlzdHMuXG4gKlxuICogQHByaXZhdGVcbiAqIEBuYW1lIGhhc1xuICogQG1lbWJlck9mIExpc3RDYWNoZVxuICogQHBhcmFtIHtzdHJpbmd9IGtleSBUaGUga2V5IG9mIHRoZSBlbnRyeSB0byBjaGVjay5cbiAqIEByZXR1cm5zIHtib29sZWFufSBSZXR1cm5zIGB0cnVlYCBpZiBhbiBlbnRyeSBmb3IgYGtleWAgZXhpc3RzLCBlbHNlIGBmYWxzZWAuXG4gKi9cbmZ1bmN0aW9uIGxpc3RDYWNoZUhhcyhrZXkpIHtcbiAgcmV0dXJuIGFzc29jSW5kZXhPZih0aGlzLl9fZGF0YV9fLCBrZXkpID4gLTE7XG59XG5cbm1vZHVsZS5leHBvcnRzID0gbGlzdENhY2hlSGFzO1xuIiwidmFyIGFzc29jSW5kZXhPZiA9IHJlcXVpcmUoJy4vX2Fzc29jSW5kZXhPZicpO1xuXG4vKipcbiAqIFNldHMgdGhlIGxpc3QgY2FjaGUgYGtleWAgdG8gYHZhbHVlYC5cbiAqXG4gKiBAcHJpdmF0ZVxuICogQG5hbWUgc2V0XG4gKiBAbWVtYmVyT2YgTGlzdENhY2hlXG4gKiBAcGFyYW0ge3N0cmluZ30ga2V5IFRoZSBrZXkgb2YgdGhlIHZhbHVlIHRvIHNldC5cbiAqIEBwYXJhbSB7Kn0gdmFsdWUgVGhlIHZhbHVlIHRvIHNldC5cbiAqIEByZXR1cm5zIHtPYmplY3R9IFJldHVybnMgdGhlIGxpc3QgY2FjaGUgaW5zdGFuY2UuXG4gKi9cbmZ1bmN0aW9uIGxpc3RDYWNoZVNldChrZXksIHZhbHVlKSB7XG4gIHZhciBkYXRhID0gdGhpcy5fX2RhdGFfXyxcbiAgICAgIGluZGV4ID0gYXNzb2NJbmRleE9mKGRhdGEsIGtleSk7XG5cbiAgaWYgKGluZGV4IDwgMCkge1xuICAgICsrdGhpcy5zaXplO1xuICAgIGRhdGEucHVzaChba2V5LCB2YWx1ZV0pO1xuICB9IGVsc2Uge1xuICAgIGRhdGFbaW5kZXhdWzFdID0gdmFsdWU7XG4gIH1cbiAgcmV0dXJuIHRoaXM7XG59XG5cbm1vZHVsZS5leHBvcnRzID0gbGlzdENhY2hlU2V0O1xuIiwidmFyIEhhc2ggPSByZXF1aXJlKCcuL19IYXNoJyksXG4gICAgTGlzdENhY2hlID0gcmVxdWlyZSgnLi9fTGlzdENhY2hlJyksXG4gICAgTWFwID0gcmVxdWlyZSgnLi9fTWFwJyk7XG5cbi8qKlxuICogUmVtb3ZlcyBhbGwga2V5LXZhbHVlIGVudHJpZXMgZnJvbSB0aGUgbWFwLlxuICpcbiAqIEBwcml2YXRlXG4gKiBAbmFtZSBjbGVhclxuICogQG1lbWJlck9mIE1hcENhY2hlXG4gKi9cbmZ1bmN0aW9uIG1hcENhY2hlQ2xlYXIoKSB7XG4gIHRoaXMuc2l6ZSA9IDA7XG4gIHRoaXMuX19kYXRhX18gPSB7XG4gICAgJ2hhc2gnOiBuZXcgSGFzaCxcbiAgICAnbWFwJzogbmV3IChNYXAgfHwgTGlzdENhY2hlKSxcbiAgICAnc3RyaW5nJzogbmV3IEhhc2hcbiAgfTtcbn1cblxubW9kdWxlLmV4cG9ydHMgPSBtYXBDYWNoZUNsZWFyO1xuIiwidmFyIGdldE1hcERhdGEgPSByZXF1aXJlKCcuL19nZXRNYXBEYXRhJyk7XG5cbi8qKlxuICogUmVtb3ZlcyBga2V5YCBhbmQgaXRzIHZhbHVlIGZyb20gdGhlIG1hcC5cbiAqXG4gKiBAcHJpdmF0ZVxuICogQG5hbWUgZGVsZXRlXG4gKiBAbWVtYmVyT2YgTWFwQ2FjaGVcbiAqIEBwYXJhbSB7c3RyaW5nfSBrZXkgVGhlIGtleSBvZiB0aGUgdmFsdWUgdG8gcmVtb3ZlLlxuICogQHJldHVybnMge2Jvb2xlYW59IFJldHVybnMgYHRydWVgIGlmIHRoZSBlbnRyeSB3YXMgcmVtb3ZlZCwgZWxzZSBgZmFsc2VgLlxuICovXG5mdW5jdGlvbiBtYXBDYWNoZURlbGV0ZShrZXkpIHtcbiAgdmFyIHJlc3VsdCA9IGdldE1hcERhdGEodGhpcywga2V5KVsnZGVsZXRlJ10oa2V5KTtcbiAgdGhpcy5zaXplIC09IHJlc3VsdCA/IDEgOiAwO1xuICByZXR1cm4gcmVzdWx0O1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IG1hcENhY2hlRGVsZXRlO1xuIiwidmFyIGdldE1hcERhdGEgPSByZXF1aXJlKCcuL19nZXRNYXBEYXRhJyk7XG5cbi8qKlxuICogR2V0cyB0aGUgbWFwIHZhbHVlIGZvciBga2V5YC5cbiAqXG4gKiBAcHJpdmF0ZVxuICogQG5hbWUgZ2V0XG4gKiBAbWVtYmVyT2YgTWFwQ2FjaGVcbiAqIEBwYXJhbSB7c3RyaW5nfSBrZXkgVGhlIGtleSBvZiB0aGUgdmFsdWUgdG8gZ2V0LlxuICogQHJldHVybnMgeyp9IFJldHVybnMgdGhlIGVudHJ5IHZhbHVlLlxuICovXG5mdW5jdGlvbiBtYXBDYWNoZUdldChrZXkpIHtcbiAgcmV0dXJuIGdldE1hcERhdGEodGhpcywga2V5KS5nZXQoa2V5KTtcbn1cblxubW9kdWxlLmV4cG9ydHMgPSBtYXBDYWNoZUdldDtcbiIsInZhciBnZXRNYXBEYXRhID0gcmVxdWlyZSgnLi9fZ2V0TWFwRGF0YScpO1xuXG4vKipcbiAqIENoZWNrcyBpZiBhIG1hcCB2YWx1ZSBmb3IgYGtleWAgZXhpc3RzLlxuICpcbiAqIEBwcml2YXRlXG4gKiBAbmFtZSBoYXNcbiAqIEBtZW1iZXJPZiBNYXBDYWNoZVxuICogQHBhcmFtIHtzdHJpbmd9IGtleSBUaGUga2V5IG9mIHRoZSBlbnRyeSB0byBjaGVjay5cbiAqIEByZXR1cm5zIHtib29sZWFufSBSZXR1cm5zIGB0cnVlYCBpZiBhbiBlbnRyeSBmb3IgYGtleWAgZXhpc3RzLCBlbHNlIGBmYWxzZWAuXG4gKi9cbmZ1bmN0aW9uIG1hcENhY2hlSGFzKGtleSkge1xuICByZXR1cm4gZ2V0TWFwRGF0YSh0aGlzLCBrZXkpLmhhcyhrZXkpO1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IG1hcENhY2hlSGFzO1xuIiwidmFyIGdldE1hcERhdGEgPSByZXF1aXJlKCcuL19nZXRNYXBEYXRhJyk7XG5cbi8qKlxuICogU2V0cyB0aGUgbWFwIGBrZXlgIHRvIGB2YWx1ZWAuXG4gKlxuICogQHByaXZhdGVcbiAqIEBuYW1lIHNldFxuICogQG1lbWJlck9mIE1hcENhY2hlXG4gKiBAcGFyYW0ge3N0cmluZ30ga2V5IFRoZSBrZXkgb2YgdGhlIHZhbHVlIHRvIHNldC5cbiAqIEBwYXJhbSB7Kn0gdmFsdWUgVGhlIHZhbHVlIHRvIHNldC5cbiAqIEByZXR1cm5zIHtPYmplY3R9IFJldHVybnMgdGhlIG1hcCBjYWNoZSBpbnN0YW5jZS5cbiAqL1xuZnVuY3Rpb24gbWFwQ2FjaGVTZXQoa2V5LCB2YWx1ZSkge1xuICB2YXIgZGF0YSA9IGdldE1hcERhdGEodGhpcywga2V5KSxcbiAgICAgIHNpemUgPSBkYXRhLnNpemU7XG5cbiAgZGF0YS5zZXQoa2V5LCB2YWx1ZSk7XG4gIHRoaXMuc2l6ZSArPSBkYXRhLnNpemUgPT0gc2l6ZSA/IDAgOiAxO1xuICByZXR1cm4gdGhpcztcbn1cblxubW9kdWxlLmV4cG9ydHMgPSBtYXBDYWNoZVNldDtcbiIsIi8qKlxuICogQ29udmVydHMgYG1hcGAgdG8gaXRzIGtleS12YWx1ZSBwYWlycy5cbiAqXG4gKiBAcHJpdmF0ZVxuICogQHBhcmFtIHtPYmplY3R9IG1hcCBUaGUgbWFwIHRvIGNvbnZlcnQuXG4gKiBAcmV0dXJucyB7QXJyYXl9IFJldHVybnMgdGhlIGtleS12YWx1ZSBwYWlycy5cbiAqL1xuZnVuY3Rpb24gbWFwVG9BcnJheShtYXApIHtcbiAgdmFyIGluZGV4ID0gLTEsXG4gICAgICByZXN1bHQgPSBBcnJheShtYXAuc2l6ZSk7XG5cbiAgbWFwLmZvckVhY2goZnVuY3Rpb24odmFsdWUsIGtleSkge1xuICAgIHJlc3VsdFsrK2luZGV4XSA9IFtrZXksIHZhbHVlXTtcbiAgfSk7XG4gIHJldHVybiByZXN1bHQ7XG59XG5cbm1vZHVsZS5leHBvcnRzID0gbWFwVG9BcnJheTtcbiIsInZhciBnZXROYXRpdmUgPSByZXF1aXJlKCcuL19nZXROYXRpdmUnKTtcblxuLyogQnVpbHQtaW4gbWV0aG9kIHJlZmVyZW5jZXMgdGhhdCBhcmUgdmVyaWZpZWQgdG8gYmUgbmF0aXZlLiAqL1xudmFyIG5hdGl2ZUNyZWF0ZSA9IGdldE5hdGl2ZShPYmplY3QsICdjcmVhdGUnKTtcblxubW9kdWxlLmV4cG9ydHMgPSBuYXRpdmVDcmVhdGU7XG4iLCJ2YXIgb3ZlckFyZyA9IHJlcXVpcmUoJy4vX292ZXJBcmcnKTtcblxuLyogQnVpbHQtaW4gbWV0aG9kIHJlZmVyZW5jZXMgZm9yIHRob3NlIHdpdGggdGhlIHNhbWUgbmFtZSBhcyBvdGhlciBgbG9kYXNoYCBtZXRob2RzLiAqL1xudmFyIG5hdGl2ZUtleXMgPSBvdmVyQXJnKE9iamVjdC5rZXlzLCBPYmplY3QpO1xuXG5tb2R1bGUuZXhwb3J0cyA9IG5hdGl2ZUtleXM7XG4iLCJ2YXIgZnJlZUdsb2JhbCA9IHJlcXVpcmUoJy4vX2ZyZWVHbG9iYWwnKTtcblxuLyoqIERldGVjdCBmcmVlIHZhcmlhYmxlIGBleHBvcnRzYC4gKi9cbnZhciBmcmVlRXhwb3J0cyA9IHR5cGVvZiBleHBvcnRzID09ICdvYmplY3QnICYmIGV4cG9ydHMgJiYgIWV4cG9ydHMubm9kZVR5cGUgJiYgZXhwb3J0cztcblxuLyoqIERldGVjdCBmcmVlIHZhcmlhYmxlIGBtb2R1bGVgLiAqL1xudmFyIGZyZWVNb2R1bGUgPSBmcmVlRXhwb3J0cyAmJiB0eXBlb2YgbW9kdWxlID09ICdvYmplY3QnICYmIG1vZHVsZSAmJiAhbW9kdWxlLm5vZGVUeXBlICYmIG1vZHVsZTtcblxuLyoqIERldGVjdCB0aGUgcG9wdWxhciBDb21tb25KUyBleHRlbnNpb24gYG1vZHVsZS5leHBvcnRzYC4gKi9cbnZhciBtb2R1bGVFeHBvcnRzID0gZnJlZU1vZHVsZSAmJiBmcmVlTW9kdWxlLmV4cG9ydHMgPT09IGZyZWVFeHBvcnRzO1xuXG4vKiogRGV0ZWN0IGZyZWUgdmFyaWFibGUgYHByb2Nlc3NgIGZyb20gTm9kZS5qcy4gKi9cbnZhciBmcmVlUHJvY2VzcyA9IG1vZHVsZUV4cG9ydHMgJiYgZnJlZUdsb2JhbC5wcm9jZXNzO1xuXG4vKiogVXNlZCB0byBhY2Nlc3MgZmFzdGVyIE5vZGUuanMgaGVscGVycy4gKi9cbnZhciBub2RlVXRpbCA9IChmdW5jdGlvbigpIHtcbiAgdHJ5IHtcbiAgICAvLyBVc2UgYHV0aWwudHlwZXNgIGZvciBOb2RlLmpzIDEwKy5cbiAgICB2YXIgdHlwZXMgPSBmcmVlTW9kdWxlICYmIGZyZWVNb2R1bGUucmVxdWlyZSAmJiBmcmVlTW9kdWxlLnJlcXVpcmUoJ3V0aWwnKS50eXBlcztcblxuICAgIGlmICh0eXBlcykge1xuICAgICAgcmV0dXJuIHR5cGVzO1xuICAgIH1cblxuICAgIC8vIExlZ2FjeSBgcHJvY2Vzcy5iaW5kaW5nKCd1dGlsJylgIGZvciBOb2RlLmpzIDwgMTAuXG4gICAgcmV0dXJuIGZyZWVQcm9jZXNzICYmIGZyZWVQcm9jZXNzLmJpbmRpbmcgJiYgZnJlZVByb2Nlc3MuYmluZGluZygndXRpbCcpO1xuICB9IGNhdGNoIChlKSB7fVxufSgpKTtcblxubW9kdWxlLmV4cG9ydHMgPSBub2RlVXRpbDtcbiIsIi8qKiBVc2VkIGZvciBidWlsdC1pbiBtZXRob2QgcmVmZXJlbmNlcy4gKi9cbnZhciBvYmplY3RQcm90byA9IE9iamVjdC5wcm90b3R5cGU7XG5cbi8qKlxuICogVXNlZCB0byByZXNvbHZlIHRoZVxuICogW2B0b1N0cmluZ1RhZ2BdKGh0dHA6Ly9lY21hLWludGVybmF0aW9uYWwub3JnL2VjbWEtMjYyLzcuMC8jc2VjLW9iamVjdC5wcm90b3R5cGUudG9zdHJpbmcpXG4gKiBvZiB2YWx1ZXMuXG4gKi9cbnZhciBuYXRpdmVPYmplY3RUb1N0cmluZyA9IG9iamVjdFByb3RvLnRvU3RyaW5nO1xuXG4vKipcbiAqIENvbnZlcnRzIGB2YWx1ZWAgdG8gYSBzdHJpbmcgdXNpbmcgYE9iamVjdC5wcm90b3R5cGUudG9TdHJpbmdgLlxuICpcbiAqIEBwcml2YXRlXG4gKiBAcGFyYW0geyp9IHZhbHVlIFRoZSB2YWx1ZSB0byBjb252ZXJ0LlxuICogQHJldHVybnMge3N0cmluZ30gUmV0dXJucyB0aGUgY29udmVydGVkIHN0cmluZy5cbiAqL1xuZnVuY3Rpb24gb2JqZWN0VG9TdHJpbmcodmFsdWUpIHtcbiAgcmV0dXJuIG5hdGl2ZU9iamVjdFRvU3RyaW5nLmNhbGwodmFsdWUpO1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IG9iamVjdFRvU3RyaW5nO1xuIiwiLyoqXG4gKiBDcmVhdGVzIGEgdW5hcnkgZnVuY3Rpb24gdGhhdCBpbnZva2VzIGBmdW5jYCB3aXRoIGl0cyBhcmd1bWVudCB0cmFuc2Zvcm1lZC5cbiAqXG4gKiBAcHJpdmF0ZVxuICogQHBhcmFtIHtGdW5jdGlvbn0gZnVuYyBUaGUgZnVuY3Rpb24gdG8gd3JhcC5cbiAqIEBwYXJhbSB7RnVuY3Rpb259IHRyYW5zZm9ybSBUaGUgYXJndW1lbnQgdHJhbnNmb3JtLlxuICogQHJldHVybnMge0Z1bmN0aW9ufSBSZXR1cm5zIHRoZSBuZXcgZnVuY3Rpb24uXG4gKi9cbmZ1bmN0aW9uIG92ZXJBcmcoZnVuYywgdHJhbnNmb3JtKSB7XG4gIHJldHVybiBmdW5jdGlvbihhcmcpIHtcbiAgICByZXR1cm4gZnVuYyh0cmFuc2Zvcm0oYXJnKSk7XG4gIH07XG59XG5cbm1vZHVsZS5leHBvcnRzID0gb3ZlckFyZztcbiIsInZhciBmcmVlR2xvYmFsID0gcmVxdWlyZSgnLi9fZnJlZUdsb2JhbCcpO1xuXG4vKiogRGV0ZWN0IGZyZWUgdmFyaWFibGUgYHNlbGZgLiAqL1xudmFyIGZyZWVTZWxmID0gdHlwZW9mIHNlbGYgPT0gJ29iamVjdCcgJiYgc2VsZiAmJiBzZWxmLk9iamVjdCA9PT0gT2JqZWN0ICYmIHNlbGY7XG5cbi8qKiBVc2VkIGFzIGEgcmVmZXJlbmNlIHRvIHRoZSBnbG9iYWwgb2JqZWN0LiAqL1xudmFyIHJvb3QgPSBmcmVlR2xvYmFsIHx8IGZyZWVTZWxmIHx8IEZ1bmN0aW9uKCdyZXR1cm4gdGhpcycpKCk7XG5cbm1vZHVsZS5leHBvcnRzID0gcm9vdDtcbiIsIi8qKiBVc2VkIHRvIHN0YW5kLWluIGZvciBgdW5kZWZpbmVkYCBoYXNoIHZhbHVlcy4gKi9cbnZhciBIQVNIX1VOREVGSU5FRCA9ICdfX2xvZGFzaF9oYXNoX3VuZGVmaW5lZF9fJztcblxuLyoqXG4gKiBBZGRzIGB2YWx1ZWAgdG8gdGhlIGFycmF5IGNhY2hlLlxuICpcbiAqIEBwcml2YXRlXG4gKiBAbmFtZSBhZGRcbiAqIEBtZW1iZXJPZiBTZXRDYWNoZVxuICogQGFsaWFzIHB1c2hcbiAqIEBwYXJhbSB7Kn0gdmFsdWUgVGhlIHZhbHVlIHRvIGNhY2hlLlxuICogQHJldHVybnMge09iamVjdH0gUmV0dXJucyB0aGUgY2FjaGUgaW5zdGFuY2UuXG4gKi9cbmZ1bmN0aW9uIHNldENhY2hlQWRkKHZhbHVlKSB7XG4gIHRoaXMuX19kYXRhX18uc2V0KHZhbHVlLCBIQVNIX1VOREVGSU5FRCk7XG4gIHJldHVybiB0aGlzO1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IHNldENhY2hlQWRkO1xuIiwiLyoqXG4gKiBDaGVja3MgaWYgYHZhbHVlYCBpcyBpbiB0aGUgYXJyYXkgY2FjaGUuXG4gKlxuICogQHByaXZhdGVcbiAqIEBuYW1lIGhhc1xuICogQG1lbWJlck9mIFNldENhY2hlXG4gKiBAcGFyYW0geyp9IHZhbHVlIFRoZSB2YWx1ZSB0byBzZWFyY2ggZm9yLlxuICogQHJldHVybnMge251bWJlcn0gUmV0dXJucyBgdHJ1ZWAgaWYgYHZhbHVlYCBpcyBmb3VuZCwgZWxzZSBgZmFsc2VgLlxuICovXG5mdW5jdGlvbiBzZXRDYWNoZUhhcyh2YWx1ZSkge1xuICByZXR1cm4gdGhpcy5fX2RhdGFfXy5oYXModmFsdWUpO1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IHNldENhY2hlSGFzO1xuIiwiLyoqXG4gKiBDb252ZXJ0cyBgc2V0YCB0byBhbiBhcnJheSBvZiBpdHMgdmFsdWVzLlxuICpcbiAqIEBwcml2YXRlXG4gKiBAcGFyYW0ge09iamVjdH0gc2V0IFRoZSBzZXQgdG8gY29udmVydC5cbiAqIEByZXR1cm5zIHtBcnJheX0gUmV0dXJucyB0aGUgdmFsdWVzLlxuICovXG5mdW5jdGlvbiBzZXRUb0FycmF5KHNldCkge1xuICB2YXIgaW5kZXggPSAtMSxcbiAgICAgIHJlc3VsdCA9IEFycmF5KHNldC5zaXplKTtcblxuICBzZXQuZm9yRWFjaChmdW5jdGlvbih2YWx1ZSkge1xuICAgIHJlc3VsdFsrK2luZGV4XSA9IHZhbHVlO1xuICB9KTtcbiAgcmV0dXJuIHJlc3VsdDtcbn1cblxubW9kdWxlLmV4cG9ydHMgPSBzZXRUb0FycmF5O1xuIiwidmFyIExpc3RDYWNoZSA9IHJlcXVpcmUoJy4vX0xpc3RDYWNoZScpO1xuXG4vKipcbiAqIFJlbW92ZXMgYWxsIGtleS12YWx1ZSBlbnRyaWVzIGZyb20gdGhlIHN0YWNrLlxuICpcbiAqIEBwcml2YXRlXG4gKiBAbmFtZSBjbGVhclxuICogQG1lbWJlck9mIFN0YWNrXG4gKi9cbmZ1bmN0aW9uIHN0YWNrQ2xlYXIoKSB7XG4gIHRoaXMuX19kYXRhX18gPSBuZXcgTGlzdENhY2hlO1xuICB0aGlzLnNpemUgPSAwO1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IHN0YWNrQ2xlYXI7XG4iLCIvKipcbiAqIFJlbW92ZXMgYGtleWAgYW5kIGl0cyB2YWx1ZSBmcm9tIHRoZSBzdGFjay5cbiAqXG4gKiBAcHJpdmF0ZVxuICogQG5hbWUgZGVsZXRlXG4gKiBAbWVtYmVyT2YgU3RhY2tcbiAqIEBwYXJhbSB7c3RyaW5nfSBrZXkgVGhlIGtleSBvZiB0aGUgdmFsdWUgdG8gcmVtb3ZlLlxuICogQHJldHVybnMge2Jvb2xlYW59IFJldHVybnMgYHRydWVgIGlmIHRoZSBlbnRyeSB3YXMgcmVtb3ZlZCwgZWxzZSBgZmFsc2VgLlxuICovXG5mdW5jdGlvbiBzdGFja0RlbGV0ZShrZXkpIHtcbiAgdmFyIGRhdGEgPSB0aGlzLl9fZGF0YV9fLFxuICAgICAgcmVzdWx0ID0gZGF0YVsnZGVsZXRlJ10oa2V5KTtcblxuICB0aGlzLnNpemUgPSBkYXRhLnNpemU7XG4gIHJldHVybiByZXN1bHQ7XG59XG5cbm1vZHVsZS5leHBvcnRzID0gc3RhY2tEZWxldGU7XG4iLCIvKipcbiAqIEdldHMgdGhlIHN0YWNrIHZhbHVlIGZvciBga2V5YC5cbiAqXG4gKiBAcHJpdmF0ZVxuICogQG5hbWUgZ2V0XG4gKiBAbWVtYmVyT2YgU3RhY2tcbiAqIEBwYXJhbSB7c3RyaW5nfSBrZXkgVGhlIGtleSBvZiB0aGUgdmFsdWUgdG8gZ2V0LlxuICogQHJldHVybnMgeyp9IFJldHVybnMgdGhlIGVudHJ5IHZhbHVlLlxuICovXG5mdW5jdGlvbiBzdGFja0dldChrZXkpIHtcbiAgcmV0dXJuIHRoaXMuX19kYXRhX18uZ2V0KGtleSk7XG59XG5cbm1vZHVsZS5leHBvcnRzID0gc3RhY2tHZXQ7XG4iLCIvKipcbiAqIENoZWNrcyBpZiBhIHN0YWNrIHZhbHVlIGZvciBga2V5YCBleGlzdHMuXG4gKlxuICogQHByaXZhdGVcbiAqIEBuYW1lIGhhc1xuICogQG1lbWJlck9mIFN0YWNrXG4gKiBAcGFyYW0ge3N0cmluZ30ga2V5IFRoZSBrZXkgb2YgdGhlIGVudHJ5IHRvIGNoZWNrLlxuICogQHJldHVybnMge2Jvb2xlYW59IFJldHVybnMgYHRydWVgIGlmIGFuIGVudHJ5IGZvciBga2V5YCBleGlzdHMsIGVsc2UgYGZhbHNlYC5cbiAqL1xuZnVuY3Rpb24gc3RhY2tIYXMoa2V5KSB7XG4gIHJldHVybiB0aGlzLl9fZGF0YV9fLmhhcyhrZXkpO1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IHN0YWNrSGFzO1xuIiwidmFyIExpc3RDYWNoZSA9IHJlcXVpcmUoJy4vX0xpc3RDYWNoZScpLFxuICAgIE1hcCA9IHJlcXVpcmUoJy4vX01hcCcpLFxuICAgIE1hcENhY2hlID0gcmVxdWlyZSgnLi9fTWFwQ2FjaGUnKTtcblxuLyoqIFVzZWQgYXMgdGhlIHNpemUgdG8gZW5hYmxlIGxhcmdlIGFycmF5IG9wdGltaXphdGlvbnMuICovXG52YXIgTEFSR0VfQVJSQVlfU0laRSA9IDIwMDtcblxuLyoqXG4gKiBTZXRzIHRoZSBzdGFjayBga2V5YCB0byBgdmFsdWVgLlxuICpcbiAqIEBwcml2YXRlXG4gKiBAbmFtZSBzZXRcbiAqIEBtZW1iZXJPZiBTdGFja1xuICogQHBhcmFtIHtzdHJpbmd9IGtleSBUaGUga2V5IG9mIHRoZSB2YWx1ZSB0byBzZXQuXG4gKiBAcGFyYW0geyp9IHZhbHVlIFRoZSB2YWx1ZSB0byBzZXQuXG4gKiBAcmV0dXJucyB7T2JqZWN0fSBSZXR1cm5zIHRoZSBzdGFjayBjYWNoZSBpbnN0YW5jZS5cbiAqL1xuZnVuY3Rpb24gc3RhY2tTZXQoa2V5LCB2YWx1ZSkge1xuICB2YXIgZGF0YSA9IHRoaXMuX19kYXRhX187XG4gIGlmIChkYXRhIGluc3RhbmNlb2YgTGlzdENhY2hlKSB7XG4gICAgdmFyIHBhaXJzID0gZGF0YS5fX2RhdGFfXztcbiAgICBpZiAoIU1hcCB8fCAocGFpcnMubGVuZ3RoIDwgTEFSR0VfQVJSQVlfU0laRSAtIDEpKSB7XG4gICAgICBwYWlycy5wdXNoKFtrZXksIHZhbHVlXSk7XG4gICAgICB0aGlzLnNpemUgPSArK2RhdGEuc2l6ZTtcbiAgICAgIHJldHVybiB0aGlzO1xuICAgIH1cbiAgICBkYXRhID0gdGhpcy5fX2RhdGFfXyA9IG5ldyBNYXBDYWNoZShwYWlycyk7XG4gIH1cbiAgZGF0YS5zZXQoa2V5LCB2YWx1ZSk7XG4gIHRoaXMuc2l6ZSA9IGRhdGEuc2l6ZTtcbiAgcmV0dXJuIHRoaXM7XG59XG5cbm1vZHVsZS5leHBvcnRzID0gc3RhY2tTZXQ7XG4iLCIvKiogVXNlZCBmb3IgYnVpbHQtaW4gbWV0aG9kIHJlZmVyZW5jZXMuICovXG52YXIgZnVuY1Byb3RvID0gRnVuY3Rpb24ucHJvdG90eXBlO1xuXG4vKiogVXNlZCB0byByZXNvbHZlIHRoZSBkZWNvbXBpbGVkIHNvdXJjZSBvZiBmdW5jdGlvbnMuICovXG52YXIgZnVuY1RvU3RyaW5nID0gZnVuY1Byb3RvLnRvU3RyaW5nO1xuXG4vKipcbiAqIENvbnZlcnRzIGBmdW5jYCB0byBpdHMgc291cmNlIGNvZGUuXG4gKlxuICogQHByaXZhdGVcbiAqIEBwYXJhbSB7RnVuY3Rpb259IGZ1bmMgVGhlIGZ1bmN0aW9uIHRvIGNvbnZlcnQuXG4gKiBAcmV0dXJucyB7c3RyaW5nfSBSZXR1cm5zIHRoZSBzb3VyY2UgY29kZS5cbiAqL1xuZnVuY3Rpb24gdG9Tb3VyY2UoZnVuYykge1xuICBpZiAoZnVuYyAhPSBudWxsKSB7XG4gICAgdHJ5IHtcbiAgICAgIHJldHVybiBmdW5jVG9TdHJpbmcuY2FsbChmdW5jKTtcbiAgICB9IGNhdGNoIChlKSB7fVxuICAgIHRyeSB7XG4gICAgICByZXR1cm4gKGZ1bmMgKyAnJyk7XG4gICAgfSBjYXRjaCAoZSkge31cbiAgfVxuICByZXR1cm4gJyc7XG59XG5cbm1vZHVsZS5leHBvcnRzID0gdG9Tb3VyY2U7XG4iLCIvKipcbiAqIFBlcmZvcm1zIGFcbiAqIFtgU2FtZVZhbHVlWmVyb2BdKGh0dHA6Ly9lY21hLWludGVybmF0aW9uYWwub3JnL2VjbWEtMjYyLzcuMC8jc2VjLXNhbWV2YWx1ZXplcm8pXG4gKiBjb21wYXJpc29uIGJldHdlZW4gdHdvIHZhbHVlcyB0byBkZXRlcm1pbmUgaWYgdGhleSBhcmUgZXF1aXZhbGVudC5cbiAqXG4gKiBAc3RhdGljXG4gKiBAbWVtYmVyT2YgX1xuICogQHNpbmNlIDQuMC4wXG4gKiBAY2F0ZWdvcnkgTGFuZ1xuICogQHBhcmFtIHsqfSB2YWx1ZSBUaGUgdmFsdWUgdG8gY29tcGFyZS5cbiAqIEBwYXJhbSB7Kn0gb3RoZXIgVGhlIG90aGVyIHZhbHVlIHRvIGNvbXBhcmUuXG4gKiBAcmV0dXJucyB7Ym9vbGVhbn0gUmV0dXJucyBgdHJ1ZWAgaWYgdGhlIHZhbHVlcyBhcmUgZXF1aXZhbGVudCwgZWxzZSBgZmFsc2VgLlxuICogQGV4YW1wbGVcbiAqXG4gKiB2YXIgb2JqZWN0ID0geyAnYSc6IDEgfTtcbiAqIHZhciBvdGhlciA9IHsgJ2EnOiAxIH07XG4gKlxuICogXy5lcShvYmplY3QsIG9iamVjdCk7XG4gKiAvLyA9PiB0cnVlXG4gKlxuICogXy5lcShvYmplY3QsIG90aGVyKTtcbiAqIC8vID0+IGZhbHNlXG4gKlxuICogXy5lcSgnYScsICdhJyk7XG4gKiAvLyA9PiB0cnVlXG4gKlxuICogXy5lcSgnYScsIE9iamVjdCgnYScpKTtcbiAqIC8vID0+IGZhbHNlXG4gKlxuICogXy5lcShOYU4sIE5hTik7XG4gKiAvLyA9PiB0cnVlXG4gKi9cbmZ1bmN0aW9uIGVxKHZhbHVlLCBvdGhlcikge1xuICByZXR1cm4gdmFsdWUgPT09IG90aGVyIHx8ICh2YWx1ZSAhPT0gdmFsdWUgJiYgb3RoZXIgIT09IG90aGVyKTtcbn1cblxubW9kdWxlLmV4cG9ydHMgPSBlcTtcbiIsInZhciBiYXNlSXNBcmd1bWVudHMgPSByZXF1aXJlKCcuL19iYXNlSXNBcmd1bWVudHMnKSxcbiAgICBpc09iamVjdExpa2UgPSByZXF1aXJlKCcuL2lzT2JqZWN0TGlrZScpO1xuXG4vKiogVXNlZCBmb3IgYnVpbHQtaW4gbWV0aG9kIHJlZmVyZW5jZXMuICovXG52YXIgb2JqZWN0UHJvdG8gPSBPYmplY3QucHJvdG90eXBlO1xuXG4vKiogVXNlZCB0byBjaGVjayBvYmplY3RzIGZvciBvd24gcHJvcGVydGllcy4gKi9cbnZhciBoYXNPd25Qcm9wZXJ0eSA9IG9iamVjdFByb3RvLmhhc093blByb3BlcnR5O1xuXG4vKiogQnVpbHQtaW4gdmFsdWUgcmVmZXJlbmNlcy4gKi9cbnZhciBwcm9wZXJ0eUlzRW51bWVyYWJsZSA9IG9iamVjdFByb3RvLnByb3BlcnR5SXNFbnVtZXJhYmxlO1xuXG4vKipcbiAqIENoZWNrcyBpZiBgdmFsdWVgIGlzIGxpa2VseSBhbiBgYXJndW1lbnRzYCBvYmplY3QuXG4gKlxuICogQHN0YXRpY1xuICogQG1lbWJlck9mIF9cbiAqIEBzaW5jZSAwLjEuMFxuICogQGNhdGVnb3J5IExhbmdcbiAqIEBwYXJhbSB7Kn0gdmFsdWUgVGhlIHZhbHVlIHRvIGNoZWNrLlxuICogQHJldHVybnMge2Jvb2xlYW59IFJldHVybnMgYHRydWVgIGlmIGB2YWx1ZWAgaXMgYW4gYGFyZ3VtZW50c2Agb2JqZWN0LFxuICogIGVsc2UgYGZhbHNlYC5cbiAqIEBleGFtcGxlXG4gKlxuICogXy5pc0FyZ3VtZW50cyhmdW5jdGlvbigpIHsgcmV0dXJuIGFyZ3VtZW50czsgfSgpKTtcbiAqIC8vID0+IHRydWVcbiAqXG4gKiBfLmlzQXJndW1lbnRzKFsxLCAyLCAzXSk7XG4gKiAvLyA9PiBmYWxzZVxuICovXG52YXIgaXNBcmd1bWVudHMgPSBiYXNlSXNBcmd1bWVudHMoZnVuY3Rpb24oKSB7IHJldHVybiBhcmd1bWVudHM7IH0oKSkgPyBiYXNlSXNBcmd1bWVudHMgOiBmdW5jdGlvbih2YWx1ZSkge1xuICByZXR1cm4gaXNPYmplY3RMaWtlKHZhbHVlKSAmJiBoYXNPd25Qcm9wZXJ0eS5jYWxsKHZhbHVlLCAnY2FsbGVlJykgJiZcbiAgICAhcHJvcGVydHlJc0VudW1lcmFibGUuY2FsbCh2YWx1ZSwgJ2NhbGxlZScpO1xufTtcblxubW9kdWxlLmV4cG9ydHMgPSBpc0FyZ3VtZW50cztcbiIsIi8qKlxuICogQ2hlY2tzIGlmIGB2YWx1ZWAgaXMgY2xhc3NpZmllZCBhcyBhbiBgQXJyYXlgIG9iamVjdC5cbiAqXG4gKiBAc3RhdGljXG4gKiBAbWVtYmVyT2YgX1xuICogQHNpbmNlIDAuMS4wXG4gKiBAY2F0ZWdvcnkgTGFuZ1xuICogQHBhcmFtIHsqfSB2YWx1ZSBUaGUgdmFsdWUgdG8gY2hlY2suXG4gKiBAcmV0dXJucyB7Ym9vbGVhbn0gUmV0dXJucyBgdHJ1ZWAgaWYgYHZhbHVlYCBpcyBhbiBhcnJheSwgZWxzZSBgZmFsc2VgLlxuICogQGV4YW1wbGVcbiAqXG4gKiBfLmlzQXJyYXkoWzEsIDIsIDNdKTtcbiAqIC8vID0+IHRydWVcbiAqXG4gKiBfLmlzQXJyYXkoZG9jdW1lbnQuYm9keS5jaGlsZHJlbik7XG4gKiAvLyA9PiBmYWxzZVxuICpcbiAqIF8uaXNBcnJheSgnYWJjJyk7XG4gKiAvLyA9PiBmYWxzZVxuICpcbiAqIF8uaXNBcnJheShfLm5vb3ApO1xuICogLy8gPT4gZmFsc2VcbiAqL1xudmFyIGlzQXJyYXkgPSBBcnJheS5pc0FycmF5O1xuXG5tb2R1bGUuZXhwb3J0cyA9IGlzQXJyYXk7XG4iLCJ2YXIgaXNGdW5jdGlvbiA9IHJlcXVpcmUoJy4vaXNGdW5jdGlvbicpLFxuICAgIGlzTGVuZ3RoID0gcmVxdWlyZSgnLi9pc0xlbmd0aCcpO1xuXG4vKipcbiAqIENoZWNrcyBpZiBgdmFsdWVgIGlzIGFycmF5LWxpa2UuIEEgdmFsdWUgaXMgY29uc2lkZXJlZCBhcnJheS1saWtlIGlmIGl0J3NcbiAqIG5vdCBhIGZ1bmN0aW9uIGFuZCBoYXMgYSBgdmFsdWUubGVuZ3RoYCB0aGF0J3MgYW4gaW50ZWdlciBncmVhdGVyIHRoYW4gb3JcbiAqIGVxdWFsIHRvIGAwYCBhbmQgbGVzcyB0aGFuIG9yIGVxdWFsIHRvIGBOdW1iZXIuTUFYX1NBRkVfSU5URUdFUmAuXG4gKlxuICogQHN0YXRpY1xuICogQG1lbWJlck9mIF9cbiAqIEBzaW5jZSA0LjAuMFxuICogQGNhdGVnb3J5IExhbmdcbiAqIEBwYXJhbSB7Kn0gdmFsdWUgVGhlIHZhbHVlIHRvIGNoZWNrLlxuICogQHJldHVybnMge2Jvb2xlYW59IFJldHVybnMgYHRydWVgIGlmIGB2YWx1ZWAgaXMgYXJyYXktbGlrZSwgZWxzZSBgZmFsc2VgLlxuICogQGV4YW1wbGVcbiAqXG4gKiBfLmlzQXJyYXlMaWtlKFsxLCAyLCAzXSk7XG4gKiAvLyA9PiB0cnVlXG4gKlxuICogXy5pc0FycmF5TGlrZShkb2N1bWVudC5ib2R5LmNoaWxkcmVuKTtcbiAqIC8vID0+IHRydWVcbiAqXG4gKiBfLmlzQXJyYXlMaWtlKCdhYmMnKTtcbiAqIC8vID0+IHRydWVcbiAqXG4gKiBfLmlzQXJyYXlMaWtlKF8ubm9vcCk7XG4gKiAvLyA9PiBmYWxzZVxuICovXG5mdW5jdGlvbiBpc0FycmF5TGlrZSh2YWx1ZSkge1xuICByZXR1cm4gdmFsdWUgIT0gbnVsbCAmJiBpc0xlbmd0aCh2YWx1ZS5sZW5ndGgpICYmICFpc0Z1bmN0aW9uKHZhbHVlKTtcbn1cblxubW9kdWxlLmV4cG9ydHMgPSBpc0FycmF5TGlrZTtcbiIsInZhciBiYXNlR2V0VGFnID0gcmVxdWlyZSgnLi9fYmFzZUdldFRhZycpLFxuICAgIGlzT2JqZWN0TGlrZSA9IHJlcXVpcmUoJy4vaXNPYmplY3RMaWtlJyk7XG5cbi8qKiBgT2JqZWN0I3RvU3RyaW5nYCByZXN1bHQgcmVmZXJlbmNlcy4gKi9cbnZhciBib29sVGFnID0gJ1tvYmplY3QgQm9vbGVhbl0nO1xuXG4vKipcbiAqIENoZWNrcyBpZiBgdmFsdWVgIGlzIGNsYXNzaWZpZWQgYXMgYSBib29sZWFuIHByaW1pdGl2ZSBvciBvYmplY3QuXG4gKlxuICogQHN0YXRpY1xuICogQG1lbWJlck9mIF9cbiAqIEBzaW5jZSAwLjEuMFxuICogQGNhdGVnb3J5IExhbmdcbiAqIEBwYXJhbSB7Kn0gdmFsdWUgVGhlIHZhbHVlIHRvIGNoZWNrLlxuICogQHJldHVybnMge2Jvb2xlYW59IFJldHVybnMgYHRydWVgIGlmIGB2YWx1ZWAgaXMgYSBib29sZWFuLCBlbHNlIGBmYWxzZWAuXG4gKiBAZXhhbXBsZVxuICpcbiAqIF8uaXNCb29sZWFuKGZhbHNlKTtcbiAqIC8vID0+IHRydWVcbiAqXG4gKiBfLmlzQm9vbGVhbihudWxsKTtcbiAqIC8vID0+IGZhbHNlXG4gKi9cbmZ1bmN0aW9uIGlzQm9vbGVhbih2YWx1ZSkge1xuICByZXR1cm4gdmFsdWUgPT09IHRydWUgfHwgdmFsdWUgPT09IGZhbHNlIHx8XG4gICAgKGlzT2JqZWN0TGlrZSh2YWx1ZSkgJiYgYmFzZUdldFRhZyh2YWx1ZSkgPT0gYm9vbFRhZyk7XG59XG5cbm1vZHVsZS5leHBvcnRzID0gaXNCb29sZWFuO1xuIiwidmFyIHJvb3QgPSByZXF1aXJlKCcuL19yb290JyksXG4gICAgc3R1YkZhbHNlID0gcmVxdWlyZSgnLi9zdHViRmFsc2UnKTtcblxuLyoqIERldGVjdCBmcmVlIHZhcmlhYmxlIGBleHBvcnRzYC4gKi9cbnZhciBmcmVlRXhwb3J0cyA9IHR5cGVvZiBleHBvcnRzID09ICdvYmplY3QnICYmIGV4cG9ydHMgJiYgIWV4cG9ydHMubm9kZVR5cGUgJiYgZXhwb3J0cztcblxuLyoqIERldGVjdCBmcmVlIHZhcmlhYmxlIGBtb2R1bGVgLiAqL1xudmFyIGZyZWVNb2R1bGUgPSBmcmVlRXhwb3J0cyAmJiB0eXBlb2YgbW9kdWxlID09ICdvYmplY3QnICYmIG1vZHVsZSAmJiAhbW9kdWxlLm5vZGVUeXBlICYmIG1vZHVsZTtcblxuLyoqIERldGVjdCB0aGUgcG9wdWxhciBDb21tb25KUyBleHRlbnNpb24gYG1vZHVsZS5leHBvcnRzYC4gKi9cbnZhciBtb2R1bGVFeHBvcnRzID0gZnJlZU1vZHVsZSAmJiBmcmVlTW9kdWxlLmV4cG9ydHMgPT09IGZyZWVFeHBvcnRzO1xuXG4vKiogQnVpbHQtaW4gdmFsdWUgcmVmZXJlbmNlcy4gKi9cbnZhciBCdWZmZXIgPSBtb2R1bGVFeHBvcnRzID8gcm9vdC5CdWZmZXIgOiB1bmRlZmluZWQ7XG5cbi8qIEJ1aWx0LWluIG1ldGhvZCByZWZlcmVuY2VzIGZvciB0aG9zZSB3aXRoIHRoZSBzYW1lIG5hbWUgYXMgb3RoZXIgYGxvZGFzaGAgbWV0aG9kcy4gKi9cbnZhciBuYXRpdmVJc0J1ZmZlciA9IEJ1ZmZlciA/IEJ1ZmZlci5pc0J1ZmZlciA6IHVuZGVmaW5lZDtcblxuLyoqXG4gKiBDaGVja3MgaWYgYHZhbHVlYCBpcyBhIGJ1ZmZlci5cbiAqXG4gKiBAc3RhdGljXG4gKiBAbWVtYmVyT2YgX1xuICogQHNpbmNlIDQuMy4wXG4gKiBAY2F0ZWdvcnkgTGFuZ1xuICogQHBhcmFtIHsqfSB2YWx1ZSBUaGUgdmFsdWUgdG8gY2hlY2suXG4gKiBAcmV0dXJucyB7Ym9vbGVhbn0gUmV0dXJucyBgdHJ1ZWAgaWYgYHZhbHVlYCBpcyBhIGJ1ZmZlciwgZWxzZSBgZmFsc2VgLlxuICogQGV4YW1wbGVcbiAqXG4gKiBfLmlzQnVmZmVyKG5ldyBCdWZmZXIoMikpO1xuICogLy8gPT4gdHJ1ZVxuICpcbiAqIF8uaXNCdWZmZXIobmV3IFVpbnQ4QXJyYXkoMikpO1xuICogLy8gPT4gZmFsc2VcbiAqL1xudmFyIGlzQnVmZmVyID0gbmF0aXZlSXNCdWZmZXIgfHwgc3R1YkZhbHNlO1xuXG5tb2R1bGUuZXhwb3J0cyA9IGlzQnVmZmVyO1xuIiwidmFyIGJhc2VJc0VxdWFsID0gcmVxdWlyZSgnLi9fYmFzZUlzRXF1YWwnKTtcblxuLyoqXG4gKiBQZXJmb3JtcyBhIGRlZXAgY29tcGFyaXNvbiBiZXR3ZWVuIHR3byB2YWx1ZXMgdG8gZGV0ZXJtaW5lIGlmIHRoZXkgYXJlXG4gKiBlcXVpdmFsZW50LlxuICpcbiAqICoqTm90ZToqKiBUaGlzIG1ldGhvZCBzdXBwb3J0cyBjb21wYXJpbmcgYXJyYXlzLCBhcnJheSBidWZmZXJzLCBib29sZWFucyxcbiAqIGRhdGUgb2JqZWN0cywgZXJyb3Igb2JqZWN0cywgbWFwcywgbnVtYmVycywgYE9iamVjdGAgb2JqZWN0cywgcmVnZXhlcyxcbiAqIHNldHMsIHN0cmluZ3MsIHN5bWJvbHMsIGFuZCB0eXBlZCBhcnJheXMuIGBPYmplY3RgIG9iamVjdHMgYXJlIGNvbXBhcmVkXG4gKiBieSB0aGVpciBvd24sIG5vdCBpbmhlcml0ZWQsIGVudW1lcmFibGUgcHJvcGVydGllcy4gRnVuY3Rpb25zIGFuZCBET01cbiAqIG5vZGVzIGFyZSBjb21wYXJlZCBieSBzdHJpY3QgZXF1YWxpdHksIGkuZS4gYD09PWAuXG4gKlxuICogQHN0YXRpY1xuICogQG1lbWJlck9mIF9cbiAqIEBzaW5jZSAwLjEuMFxuICogQGNhdGVnb3J5IExhbmdcbiAqIEBwYXJhbSB7Kn0gdmFsdWUgVGhlIHZhbHVlIHRvIGNvbXBhcmUuXG4gKiBAcGFyYW0geyp9IG90aGVyIFRoZSBvdGhlciB2YWx1ZSB0byBjb21wYXJlLlxuICogQHJldHVybnMge2Jvb2xlYW59IFJldHVybnMgYHRydWVgIGlmIHRoZSB2YWx1ZXMgYXJlIGVxdWl2YWxlbnQsIGVsc2UgYGZhbHNlYC5cbiAqIEBleGFtcGxlXG4gKlxuICogdmFyIG9iamVjdCA9IHsgJ2EnOiAxIH07XG4gKiB2YXIgb3RoZXIgPSB7ICdhJzogMSB9O1xuICpcbiAqIF8uaXNFcXVhbChvYmplY3QsIG90aGVyKTtcbiAqIC8vID0+IHRydWVcbiAqXG4gKiBvYmplY3QgPT09IG90aGVyO1xuICogLy8gPT4gZmFsc2VcbiAqL1xuZnVuY3Rpb24gaXNFcXVhbCh2YWx1ZSwgb3RoZXIpIHtcbiAgcmV0dXJuIGJhc2VJc0VxdWFsKHZhbHVlLCBvdGhlcik7XG59XG5cbm1vZHVsZS5leHBvcnRzID0gaXNFcXVhbDtcbiIsInZhciBiYXNlR2V0VGFnID0gcmVxdWlyZSgnLi9fYmFzZUdldFRhZycpLFxuICAgIGlzT2JqZWN0ID0gcmVxdWlyZSgnLi9pc09iamVjdCcpO1xuXG4vKiogYE9iamVjdCN0b1N0cmluZ2AgcmVzdWx0IHJlZmVyZW5jZXMuICovXG52YXIgYXN5bmNUYWcgPSAnW29iamVjdCBBc3luY0Z1bmN0aW9uXScsXG4gICAgZnVuY1RhZyA9ICdbb2JqZWN0IEZ1bmN0aW9uXScsXG4gICAgZ2VuVGFnID0gJ1tvYmplY3QgR2VuZXJhdG9yRnVuY3Rpb25dJyxcbiAgICBwcm94eVRhZyA9ICdbb2JqZWN0IFByb3h5XSc7XG5cbi8qKlxuICogQ2hlY2tzIGlmIGB2YWx1ZWAgaXMgY2xhc3NpZmllZCBhcyBhIGBGdW5jdGlvbmAgb2JqZWN0LlxuICpcbiAqIEBzdGF0aWNcbiAqIEBtZW1iZXJPZiBfXG4gKiBAc2luY2UgMC4xLjBcbiAqIEBjYXRlZ29yeSBMYW5nXG4gKiBAcGFyYW0geyp9IHZhbHVlIFRoZSB2YWx1ZSB0byBjaGVjay5cbiAqIEByZXR1cm5zIHtib29sZWFufSBSZXR1cm5zIGB0cnVlYCBpZiBgdmFsdWVgIGlzIGEgZnVuY3Rpb24sIGVsc2UgYGZhbHNlYC5cbiAqIEBleGFtcGxlXG4gKlxuICogXy5pc0Z1bmN0aW9uKF8pO1xuICogLy8gPT4gdHJ1ZVxuICpcbiAqIF8uaXNGdW5jdGlvbigvYWJjLyk7XG4gKiAvLyA9PiBmYWxzZVxuICovXG5mdW5jdGlvbiBpc0Z1bmN0aW9uKHZhbHVlKSB7XG4gIGlmICghaXNPYmplY3QodmFsdWUpKSB7XG4gICAgcmV0dXJuIGZhbHNlO1xuICB9XG4gIC8vIFRoZSB1c2Ugb2YgYE9iamVjdCN0b1N0cmluZ2AgYXZvaWRzIGlzc3VlcyB3aXRoIHRoZSBgdHlwZW9mYCBvcGVyYXRvclxuICAvLyBpbiBTYWZhcmkgOSB3aGljaCByZXR1cm5zICdvYmplY3QnIGZvciB0eXBlZCBhcnJheXMgYW5kIG90aGVyIGNvbnN0cnVjdG9ycy5cbiAgdmFyIHRhZyA9IGJhc2VHZXRUYWcodmFsdWUpO1xuICByZXR1cm4gdGFnID09IGZ1bmNUYWcgfHwgdGFnID09IGdlblRhZyB8fCB0YWcgPT0gYXN5bmNUYWcgfHwgdGFnID09IHByb3h5VGFnO1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IGlzRnVuY3Rpb247XG4iLCIvKiogVXNlZCBhcyByZWZlcmVuY2VzIGZvciB2YXJpb3VzIGBOdW1iZXJgIGNvbnN0YW50cy4gKi9cbnZhciBNQVhfU0FGRV9JTlRFR0VSID0gOTAwNzE5OTI1NDc0MDk5MTtcblxuLyoqXG4gKiBDaGVja3MgaWYgYHZhbHVlYCBpcyBhIHZhbGlkIGFycmF5LWxpa2UgbGVuZ3RoLlxuICpcbiAqICoqTm90ZToqKiBUaGlzIG1ldGhvZCBpcyBsb29zZWx5IGJhc2VkIG9uXG4gKiBbYFRvTGVuZ3RoYF0oaHR0cDovL2VjbWEtaW50ZXJuYXRpb25hbC5vcmcvZWNtYS0yNjIvNy4wLyNzZWMtdG9sZW5ndGgpLlxuICpcbiAqIEBzdGF0aWNcbiAqIEBtZW1iZXJPZiBfXG4gKiBAc2luY2UgNC4wLjBcbiAqIEBjYXRlZ29yeSBMYW5nXG4gKiBAcGFyYW0geyp9IHZhbHVlIFRoZSB2YWx1ZSB0byBjaGVjay5cbiAqIEByZXR1cm5zIHtib29sZWFufSBSZXR1cm5zIGB0cnVlYCBpZiBgdmFsdWVgIGlzIGEgdmFsaWQgbGVuZ3RoLCBlbHNlIGBmYWxzZWAuXG4gKiBAZXhhbXBsZVxuICpcbiAqIF8uaXNMZW5ndGgoMyk7XG4gKiAvLyA9PiB0cnVlXG4gKlxuICogXy5pc0xlbmd0aChOdW1iZXIuTUlOX1ZBTFVFKTtcbiAqIC8vID0+IGZhbHNlXG4gKlxuICogXy5pc0xlbmd0aChJbmZpbml0eSk7XG4gKiAvLyA9PiBmYWxzZVxuICpcbiAqIF8uaXNMZW5ndGgoJzMnKTtcbiAqIC8vID0+IGZhbHNlXG4gKi9cbmZ1bmN0aW9uIGlzTGVuZ3RoKHZhbHVlKSB7XG4gIHJldHVybiB0eXBlb2YgdmFsdWUgPT0gJ251bWJlcicgJiZcbiAgICB2YWx1ZSA+IC0xICYmIHZhbHVlICUgMSA9PSAwICYmIHZhbHVlIDw9IE1BWF9TQUZFX0lOVEVHRVI7XG59XG5cbm1vZHVsZS5leHBvcnRzID0gaXNMZW5ndGg7XG4iLCIvKipcbiAqIENoZWNrcyBpZiBgdmFsdWVgIGlzIGBudWxsYC5cbiAqXG4gKiBAc3RhdGljXG4gKiBAbWVtYmVyT2YgX1xuICogQHNpbmNlIDAuMS4wXG4gKiBAY2F0ZWdvcnkgTGFuZ1xuICogQHBhcmFtIHsqfSB2YWx1ZSBUaGUgdmFsdWUgdG8gY2hlY2suXG4gKiBAcmV0dXJucyB7Ym9vbGVhbn0gUmV0dXJucyBgdHJ1ZWAgaWYgYHZhbHVlYCBpcyBgbnVsbGAsIGVsc2UgYGZhbHNlYC5cbiAqIEBleGFtcGxlXG4gKlxuICogXy5pc051bGwobnVsbCk7XG4gKiAvLyA9PiB0cnVlXG4gKlxuICogXy5pc051bGwodm9pZCAwKTtcbiAqIC8vID0+IGZhbHNlXG4gKi9cbmZ1bmN0aW9uIGlzTnVsbCh2YWx1ZSkge1xuICByZXR1cm4gdmFsdWUgPT09IG51bGw7XG59XG5cbm1vZHVsZS5leHBvcnRzID0gaXNOdWxsO1xuIiwidmFyIGJhc2VHZXRUYWcgPSByZXF1aXJlKCcuL19iYXNlR2V0VGFnJyksXG4gICAgaXNPYmplY3RMaWtlID0gcmVxdWlyZSgnLi9pc09iamVjdExpa2UnKTtcblxuLyoqIGBPYmplY3QjdG9TdHJpbmdgIHJlc3VsdCByZWZlcmVuY2VzLiAqL1xudmFyIG51bWJlclRhZyA9ICdbb2JqZWN0IE51bWJlcl0nO1xuXG4vKipcbiAqIENoZWNrcyBpZiBgdmFsdWVgIGlzIGNsYXNzaWZpZWQgYXMgYSBgTnVtYmVyYCBwcmltaXRpdmUgb3Igb2JqZWN0LlxuICpcbiAqICoqTm90ZToqKiBUbyBleGNsdWRlIGBJbmZpbml0eWAsIGAtSW5maW5pdHlgLCBhbmQgYE5hTmAsIHdoaWNoIGFyZVxuICogY2xhc3NpZmllZCBhcyBudW1iZXJzLCB1c2UgdGhlIGBfLmlzRmluaXRlYCBtZXRob2QuXG4gKlxuICogQHN0YXRpY1xuICogQG1lbWJlck9mIF9cbiAqIEBzaW5jZSAwLjEuMFxuICogQGNhdGVnb3J5IExhbmdcbiAqIEBwYXJhbSB7Kn0gdmFsdWUgVGhlIHZhbHVlIHRvIGNoZWNrLlxuICogQHJldHVybnMge2Jvb2xlYW59IFJldHVybnMgYHRydWVgIGlmIGB2YWx1ZWAgaXMgYSBudW1iZXIsIGVsc2UgYGZhbHNlYC5cbiAqIEBleGFtcGxlXG4gKlxuICogXy5pc051bWJlcigzKTtcbiAqIC8vID0+IHRydWVcbiAqXG4gKiBfLmlzTnVtYmVyKE51bWJlci5NSU5fVkFMVUUpO1xuICogLy8gPT4gdHJ1ZVxuICpcbiAqIF8uaXNOdW1iZXIoSW5maW5pdHkpO1xuICogLy8gPT4gdHJ1ZVxuICpcbiAqIF8uaXNOdW1iZXIoJzMnKTtcbiAqIC8vID0+IGZhbHNlXG4gKi9cbmZ1bmN0aW9uIGlzTnVtYmVyKHZhbHVlKSB7XG4gIHJldHVybiB0eXBlb2YgdmFsdWUgPT0gJ251bWJlcicgfHxcbiAgICAoaXNPYmplY3RMaWtlKHZhbHVlKSAmJiBiYXNlR2V0VGFnKHZhbHVlKSA9PSBudW1iZXJUYWcpO1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IGlzTnVtYmVyO1xuIiwiLyoqXG4gKiBDaGVja3MgaWYgYHZhbHVlYCBpcyB0aGVcbiAqIFtsYW5ndWFnZSB0eXBlXShodHRwOi8vd3d3LmVjbWEtaW50ZXJuYXRpb25hbC5vcmcvZWNtYS0yNjIvNy4wLyNzZWMtZWNtYXNjcmlwdC1sYW5ndWFnZS10eXBlcylcbiAqIG9mIGBPYmplY3RgLiAoZS5nLiBhcnJheXMsIGZ1bmN0aW9ucywgb2JqZWN0cywgcmVnZXhlcywgYG5ldyBOdW1iZXIoMClgLCBhbmQgYG5ldyBTdHJpbmcoJycpYClcbiAqXG4gKiBAc3RhdGljXG4gKiBAbWVtYmVyT2YgX1xuICogQHNpbmNlIDAuMS4wXG4gKiBAY2F0ZWdvcnkgTGFuZ1xuICogQHBhcmFtIHsqfSB2YWx1ZSBUaGUgdmFsdWUgdG8gY2hlY2suXG4gKiBAcmV0dXJucyB7Ym9vbGVhbn0gUmV0dXJucyBgdHJ1ZWAgaWYgYHZhbHVlYCBpcyBhbiBvYmplY3QsIGVsc2UgYGZhbHNlYC5cbiAqIEBleGFtcGxlXG4gKlxuICogXy5pc09iamVjdCh7fSk7XG4gKiAvLyA9PiB0cnVlXG4gKlxuICogXy5pc09iamVjdChbMSwgMiwgM10pO1xuICogLy8gPT4gdHJ1ZVxuICpcbiAqIF8uaXNPYmplY3QoXy5ub29wKTtcbiAqIC8vID0+IHRydWVcbiAqXG4gKiBfLmlzT2JqZWN0KG51bGwpO1xuICogLy8gPT4gZmFsc2VcbiAqL1xuZnVuY3Rpb24gaXNPYmplY3QodmFsdWUpIHtcbiAgdmFyIHR5cGUgPSB0eXBlb2YgdmFsdWU7XG4gIHJldHVybiB2YWx1ZSAhPSBudWxsICYmICh0eXBlID09ICdvYmplY3QnIHx8IHR5cGUgPT0gJ2Z1bmN0aW9uJyk7XG59XG5cbm1vZHVsZS5leHBvcnRzID0gaXNPYmplY3Q7XG4iLCIvKipcbiAqIENoZWNrcyBpZiBgdmFsdWVgIGlzIG9iamVjdC1saWtlLiBBIHZhbHVlIGlzIG9iamVjdC1saWtlIGlmIGl0J3Mgbm90IGBudWxsYFxuICogYW5kIGhhcyBhIGB0eXBlb2ZgIHJlc3VsdCBvZiBcIm9iamVjdFwiLlxuICpcbiAqIEBzdGF0aWNcbiAqIEBtZW1iZXJPZiBfXG4gKiBAc2luY2UgNC4wLjBcbiAqIEBjYXRlZ29yeSBMYW5nXG4gKiBAcGFyYW0geyp9IHZhbHVlIFRoZSB2YWx1ZSB0byBjaGVjay5cbiAqIEByZXR1cm5zIHtib29sZWFufSBSZXR1cm5zIGB0cnVlYCBpZiBgdmFsdWVgIGlzIG9iamVjdC1saWtlLCBlbHNlIGBmYWxzZWAuXG4gKiBAZXhhbXBsZVxuICpcbiAqIF8uaXNPYmplY3RMaWtlKHt9KTtcbiAqIC8vID0+IHRydWVcbiAqXG4gKiBfLmlzT2JqZWN0TGlrZShbMSwgMiwgM10pO1xuICogLy8gPT4gdHJ1ZVxuICpcbiAqIF8uaXNPYmplY3RMaWtlKF8ubm9vcCk7XG4gKiAvLyA9PiBmYWxzZVxuICpcbiAqIF8uaXNPYmplY3RMaWtlKG51bGwpO1xuICogLy8gPT4gZmFsc2VcbiAqL1xuZnVuY3Rpb24gaXNPYmplY3RMaWtlKHZhbHVlKSB7XG4gIHJldHVybiB2YWx1ZSAhPSBudWxsICYmIHR5cGVvZiB2YWx1ZSA9PSAnb2JqZWN0Jztcbn1cblxubW9kdWxlLmV4cG9ydHMgPSBpc09iamVjdExpa2U7XG4iLCJ2YXIgYmFzZUdldFRhZyA9IHJlcXVpcmUoJy4vX2Jhc2VHZXRUYWcnKSxcbiAgICBpc0FycmF5ID0gcmVxdWlyZSgnLi9pc0FycmF5JyksXG4gICAgaXNPYmplY3RMaWtlID0gcmVxdWlyZSgnLi9pc09iamVjdExpa2UnKTtcblxuLyoqIGBPYmplY3QjdG9TdHJpbmdgIHJlc3VsdCByZWZlcmVuY2VzLiAqL1xudmFyIHN0cmluZ1RhZyA9ICdbb2JqZWN0IFN0cmluZ10nO1xuXG4vKipcbiAqIENoZWNrcyBpZiBgdmFsdWVgIGlzIGNsYXNzaWZpZWQgYXMgYSBgU3RyaW5nYCBwcmltaXRpdmUgb3Igb2JqZWN0LlxuICpcbiAqIEBzdGF0aWNcbiAqIEBzaW5jZSAwLjEuMFxuICogQG1lbWJlck9mIF9cbiAqIEBjYXRlZ29yeSBMYW5nXG4gKiBAcGFyYW0geyp9IHZhbHVlIFRoZSB2YWx1ZSB0byBjaGVjay5cbiAqIEByZXR1cm5zIHtib29sZWFufSBSZXR1cm5zIGB0cnVlYCBpZiBgdmFsdWVgIGlzIGEgc3RyaW5nLCBlbHNlIGBmYWxzZWAuXG4gKiBAZXhhbXBsZVxuICpcbiAqIF8uaXNTdHJpbmcoJ2FiYycpO1xuICogLy8gPT4gdHJ1ZVxuICpcbiAqIF8uaXNTdHJpbmcoMSk7XG4gKiAvLyA9PiBmYWxzZVxuICovXG5mdW5jdGlvbiBpc1N0cmluZyh2YWx1ZSkge1xuICByZXR1cm4gdHlwZW9mIHZhbHVlID09ICdzdHJpbmcnIHx8XG4gICAgKCFpc0FycmF5KHZhbHVlKSAmJiBpc09iamVjdExpa2UodmFsdWUpICYmIGJhc2VHZXRUYWcodmFsdWUpID09IHN0cmluZ1RhZyk7XG59XG5cbm1vZHVsZS5leHBvcnRzID0gaXNTdHJpbmc7XG4iLCJ2YXIgYmFzZUlzVHlwZWRBcnJheSA9IHJlcXVpcmUoJy4vX2Jhc2VJc1R5cGVkQXJyYXknKSxcbiAgICBiYXNlVW5hcnkgPSByZXF1aXJlKCcuL19iYXNlVW5hcnknKSxcbiAgICBub2RlVXRpbCA9IHJlcXVpcmUoJy4vX25vZGVVdGlsJyk7XG5cbi8qIE5vZGUuanMgaGVscGVyIHJlZmVyZW5jZXMuICovXG52YXIgbm9kZUlzVHlwZWRBcnJheSA9IG5vZGVVdGlsICYmIG5vZGVVdGlsLmlzVHlwZWRBcnJheTtcblxuLyoqXG4gKiBDaGVja3MgaWYgYHZhbHVlYCBpcyBjbGFzc2lmaWVkIGFzIGEgdHlwZWQgYXJyYXkuXG4gKlxuICogQHN0YXRpY1xuICogQG1lbWJlck9mIF9cbiAqIEBzaW5jZSAzLjAuMFxuICogQGNhdGVnb3J5IExhbmdcbiAqIEBwYXJhbSB7Kn0gdmFsdWUgVGhlIHZhbHVlIHRvIGNoZWNrLlxuICogQHJldHVybnMge2Jvb2xlYW59IFJldHVybnMgYHRydWVgIGlmIGB2YWx1ZWAgaXMgYSB0eXBlZCBhcnJheSwgZWxzZSBgZmFsc2VgLlxuICogQGV4YW1wbGVcbiAqXG4gKiBfLmlzVHlwZWRBcnJheShuZXcgVWludDhBcnJheSk7XG4gKiAvLyA9PiB0cnVlXG4gKlxuICogXy5pc1R5cGVkQXJyYXkoW10pO1xuICogLy8gPT4gZmFsc2VcbiAqL1xudmFyIGlzVHlwZWRBcnJheSA9IG5vZGVJc1R5cGVkQXJyYXkgPyBiYXNlVW5hcnkobm9kZUlzVHlwZWRBcnJheSkgOiBiYXNlSXNUeXBlZEFycmF5O1xuXG5tb2R1bGUuZXhwb3J0cyA9IGlzVHlwZWRBcnJheTtcbiIsInZhciBhcnJheUxpa2VLZXlzID0gcmVxdWlyZSgnLi9fYXJyYXlMaWtlS2V5cycpLFxuICAgIGJhc2VLZXlzID0gcmVxdWlyZSgnLi9fYmFzZUtleXMnKSxcbiAgICBpc0FycmF5TGlrZSA9IHJlcXVpcmUoJy4vaXNBcnJheUxpa2UnKTtcblxuLyoqXG4gKiBDcmVhdGVzIGFuIGFycmF5IG9mIHRoZSBvd24gZW51bWVyYWJsZSBwcm9wZXJ0eSBuYW1lcyBvZiBgb2JqZWN0YC5cbiAqXG4gKiAqKk5vdGU6KiogTm9uLW9iamVjdCB2YWx1ZXMgYXJlIGNvZXJjZWQgdG8gb2JqZWN0cy4gU2VlIHRoZVxuICogW0VTIHNwZWNdKGh0dHA6Ly9lY21hLWludGVybmF0aW9uYWwub3JnL2VjbWEtMjYyLzcuMC8jc2VjLW9iamVjdC5rZXlzKVxuICogZm9yIG1vcmUgZGV0YWlscy5cbiAqXG4gKiBAc3RhdGljXG4gKiBAc2luY2UgMC4xLjBcbiAqIEBtZW1iZXJPZiBfXG4gKiBAY2F0ZWdvcnkgT2JqZWN0XG4gKiBAcGFyYW0ge09iamVjdH0gb2JqZWN0IFRoZSBvYmplY3QgdG8gcXVlcnkuXG4gKiBAcmV0dXJucyB7QXJyYXl9IFJldHVybnMgdGhlIGFycmF5IG9mIHByb3BlcnR5IG5hbWVzLlxuICogQGV4YW1wbGVcbiAqXG4gKiBmdW5jdGlvbiBGb28oKSB7XG4gKiAgIHRoaXMuYSA9IDE7XG4gKiAgIHRoaXMuYiA9IDI7XG4gKiB9XG4gKlxuICogRm9vLnByb3RvdHlwZS5jID0gMztcbiAqXG4gKiBfLmtleXMobmV3IEZvbyk7XG4gKiAvLyA9PiBbJ2EnLCAnYiddIChpdGVyYXRpb24gb3JkZXIgaXMgbm90IGd1YXJhbnRlZWQpXG4gKlxuICogXy5rZXlzKCdoaScpO1xuICogLy8gPT4gWycwJywgJzEnXVxuICovXG5mdW5jdGlvbiBrZXlzKG9iamVjdCkge1xuICByZXR1cm4gaXNBcnJheUxpa2Uob2JqZWN0KSA/IGFycmF5TGlrZUtleXMob2JqZWN0KSA6IGJhc2VLZXlzKG9iamVjdCk7XG59XG5cbm1vZHVsZS5leHBvcnRzID0ga2V5cztcbiIsIi8qKiBFcnJvciBtZXNzYWdlIGNvbnN0YW50cy4gKi9cbnZhciBGVU5DX0VSUk9SX1RFWFQgPSAnRXhwZWN0ZWQgYSBmdW5jdGlvbic7XG5cbi8qKlxuICogQ3JlYXRlcyBhIGZ1bmN0aW9uIHRoYXQgbmVnYXRlcyB0aGUgcmVzdWx0IG9mIHRoZSBwcmVkaWNhdGUgYGZ1bmNgLiBUaGVcbiAqIGBmdW5jYCBwcmVkaWNhdGUgaXMgaW52b2tlZCB3aXRoIHRoZSBgdGhpc2AgYmluZGluZyBhbmQgYXJndW1lbnRzIG9mIHRoZVxuICogY3JlYXRlZCBmdW5jdGlvbi5cbiAqXG4gKiBAc3RhdGljXG4gKiBAbWVtYmVyT2YgX1xuICogQHNpbmNlIDMuMC4wXG4gKiBAY2F0ZWdvcnkgRnVuY3Rpb25cbiAqIEBwYXJhbSB7RnVuY3Rpb259IHByZWRpY2F0ZSBUaGUgcHJlZGljYXRlIHRvIG5lZ2F0ZS5cbiAqIEByZXR1cm5zIHtGdW5jdGlvbn0gUmV0dXJucyB0aGUgbmV3IG5lZ2F0ZWQgZnVuY3Rpb24uXG4gKiBAZXhhbXBsZVxuICpcbiAqIGZ1bmN0aW9uIGlzRXZlbihuKSB7XG4gKiAgIHJldHVybiBuICUgMiA9PSAwO1xuICogfVxuICpcbiAqIF8uZmlsdGVyKFsxLCAyLCAzLCA0LCA1LCA2XSwgXy5uZWdhdGUoaXNFdmVuKSk7XG4gKiAvLyA9PiBbMSwgMywgNV1cbiAqL1xuZnVuY3Rpb24gbmVnYXRlKHByZWRpY2F0ZSkge1xuICBpZiAodHlwZW9mIHByZWRpY2F0ZSAhPSAnZnVuY3Rpb24nKSB7XG4gICAgdGhyb3cgbmV3IFR5cGVFcnJvcihGVU5DX0VSUk9SX1RFWFQpO1xuICB9XG4gIHJldHVybiBmdW5jdGlvbigpIHtcbiAgICB2YXIgYXJncyA9IGFyZ3VtZW50cztcbiAgICBzd2l0Y2ggKGFyZ3MubGVuZ3RoKSB7XG4gICAgICBjYXNlIDA6IHJldHVybiAhcHJlZGljYXRlLmNhbGwodGhpcyk7XG4gICAgICBjYXNlIDE6IHJldHVybiAhcHJlZGljYXRlLmNhbGwodGhpcywgYXJnc1swXSk7XG4gICAgICBjYXNlIDI6IHJldHVybiAhcHJlZGljYXRlLmNhbGwodGhpcywgYXJnc1swXSwgYXJnc1sxXSk7XG4gICAgICBjYXNlIDM6IHJldHVybiAhcHJlZGljYXRlLmNhbGwodGhpcywgYXJnc1swXSwgYXJnc1sxXSwgYXJnc1syXSk7XG4gICAgfVxuICAgIHJldHVybiAhcHJlZGljYXRlLmFwcGx5KHRoaXMsIGFyZ3MpO1xuICB9O1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IG5lZ2F0ZTtcbiIsIi8qKlxuICogVGhpcyBtZXRob2QgcmV0dXJucyBhIG5ldyBlbXB0eSBhcnJheS5cbiAqXG4gKiBAc3RhdGljXG4gKiBAbWVtYmVyT2YgX1xuICogQHNpbmNlIDQuMTMuMFxuICogQGNhdGVnb3J5IFV0aWxcbiAqIEByZXR1cm5zIHtBcnJheX0gUmV0dXJucyB0aGUgbmV3IGVtcHR5IGFycmF5LlxuICogQGV4YW1wbGVcbiAqXG4gKiB2YXIgYXJyYXlzID0gXy50aW1lcygyLCBfLnN0dWJBcnJheSk7XG4gKlxuICogY29uc29sZS5sb2coYXJyYXlzKTtcbiAqIC8vID0+IFtbXSwgW11dXG4gKlxuICogY29uc29sZS5sb2coYXJyYXlzWzBdID09PSBhcnJheXNbMV0pO1xuICogLy8gPT4gZmFsc2VcbiAqL1xuZnVuY3Rpb24gc3R1YkFycmF5KCkge1xuICByZXR1cm4gW107XG59XG5cbm1vZHVsZS5leHBvcnRzID0gc3R1YkFycmF5O1xuIiwiLyoqXG4gKiBUaGlzIG1ldGhvZCByZXR1cm5zIGBmYWxzZWAuXG4gKlxuICogQHN0YXRpY1xuICogQG1lbWJlck9mIF9cbiAqIEBzaW5jZSA0LjEzLjBcbiAqIEBjYXRlZ29yeSBVdGlsXG4gKiBAcmV0dXJucyB7Ym9vbGVhbn0gUmV0dXJucyBgZmFsc2VgLlxuICogQGV4YW1wbGVcbiAqXG4gKiBfLnRpbWVzKDIsIF8uc3R1YkZhbHNlKTtcbiAqIC8vID0+IFtmYWxzZSwgZmFsc2VdXG4gKi9cbmZ1bmN0aW9uIHN0dWJGYWxzZSgpIHtcbiAgcmV0dXJuIGZhbHNlO1xufVxuXG5tb2R1bGUuZXhwb3J0cyA9IHN0dWJGYWxzZTtcbiJdfQ==
