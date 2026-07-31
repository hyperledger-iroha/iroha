'use strict';

Object.defineProperty(exports, '__esModule', { value: true });

function _interopDefault (ex) { return (ex && (typeof ex === 'object') && 'default' in ex) ? ex['default'] : ex; }

var React = _interopDefault(require('react'));
var immutable = require('immutable');

function _typeof(obj) {
  if (typeof Symbol === "function" && typeof Symbol.iterator === "symbol") {
    _typeof = function (obj) {
      return typeof obj;
    };
  } else {
    _typeof = function (obj) {
      return obj && typeof Symbol === "function" && obj.constructor === Symbol && obj !== Symbol.prototype ? "symbol" : typeof obj;
    };
  }

  return _typeof(obj);
}

function _classCallCheck(instance, Constructor) {
  if (!(instance instanceof Constructor)) {
    throw new TypeError("Cannot call a class as a function");
  }
}

function _defineProperties(target, props) {
  for (var i = 0; i < props.length; i++) {
    var descriptor = props[i];
    descriptor.enumerable = descriptor.enumerable || false;
    descriptor.configurable = true;
    if ("value" in descriptor) descriptor.writable = true;
    Object.defineProperty(target, descriptor.key, descriptor);
  }
}

function _createClass(Constructor, protoProps, staticProps) {
  if (protoProps) _defineProperties(Constructor.prototype, protoProps);
  if (staticProps) _defineProperties(Constructor, staticProps);
  return Constructor;
}

function _defineProperty(obj, key, value) {
  if (key in obj) {
    Object.defineProperty(obj, key, {
      value: value,
      enumerable: true,
      configurable: true,
      writable: true
    });
  } else {
    obj[key] = value;
  }

  return obj;
}

function ownKeys(object, enumerableOnly) {
  var keys = Object.keys(object);

  if (Object.getOwnPropertySymbols) {
    var symbols = Object.getOwnPropertySymbols(object);
    if (enumerableOnly) symbols = symbols.filter(function (sym) {
      return Object.getOwnPropertyDescriptor(object, sym).enumerable;
    });
    keys.push.apply(keys, symbols);
  }

  return keys;
}

function _objectSpread2(target) {
  for (var i = 1; i < arguments.length; i++) {
    var source = arguments[i] != null ? arguments[i] : {};

    if (i % 2) {
      ownKeys(source, true).forEach(function (key) {
        _defineProperty(target, key, source[key]);
      });
    } else if (Object.getOwnPropertyDescriptors) {
      Object.defineProperties(target, Object.getOwnPropertyDescriptors(source));
    } else {
      ownKeys(source).forEach(function (key) {
        Object.defineProperty(target, key, Object.getOwnPropertyDescriptor(source, key));
      });
    }
  }

  return target;
}

function _inherits(subClass, superClass) {
  if (typeof superClass !== "function" && superClass !== null) {
    throw new TypeError("Super expression must either be null or a function");
  }

  subClass.prototype = Object.create(superClass && superClass.prototype, {
    constructor: {
      value: subClass,
      writable: true,
      configurable: true
    }
  });
  if (superClass) _setPrototypeOf(subClass, superClass);
}

function _getPrototypeOf(o) {
  _getPrototypeOf = Object.setPrototypeOf ? Object.getPrototypeOf : function _getPrototypeOf(o) {
    return o.__proto__ || Object.getPrototypeOf(o);
  };
  return _getPrototypeOf(o);
}

function _setPrototypeOf(o, p) {
  _setPrototypeOf = Object.setPrototypeOf || function _setPrototypeOf(o, p) {
    o.__proto__ = p;
    return o;
  };

  return _setPrototypeOf(o, p);
}

function _assertThisInitialized(self) {
  if (self === void 0) {
    throw new ReferenceError("this hasn't been initialised - super() hasn't been called");
  }

  return self;
}

function _possibleConstructorReturn(self, call) {
  if (call && (typeof call === "object" || typeof call === "function")) {
    return call;
  }

  return _assertThisInitialized(self);
}

var NOT_SET = {};
function isMapLike(collection) {
  return collection !== null && _typeof(collection) === 'object' && typeof collection.get === 'function' && typeof collection.has === 'function';
}

function isInvalid(collection) {
  return collection === null || collection === undefined;
}

function get(collection, key, notSetValue) {
  if (isInvalid(collection)) {
    return notSetValue;
  }

  if (isMapLike(collection)) {
    return collection.has(key) ? collection.get(key) : notSetValue;
  }

  return hasOwnProperty.call(collection, key) ? collection[key] : notSetValue;
}
function getIn(collection, keyPath, notSetValue) {
  var i = 0;

  while (i !== keyPath.length) {
    collection = get(collection, keyPath[i++], NOT_SET);

    if (collection === NOT_SET) {
      return notSetValue;
    }
  }

  return collection;
}

function check(config) {
  var prev = arguments.length > 1 && arguments[1] !== undefined ? arguments[1] : {};
  var next = arguments.length > 2 && arguments[2] !== undefined ? arguments[2] : {};
  var checkItem = createChecker(prev, next);
  var checklist = config || Object.keys(_objectSpread2({}, next, {}, prev));
  return checklist.every(checkItem);
}

function createChecker(prev, next) {
  return function (name) {
    if (typeof name === 'string') {
      return immutable.is(next[name], prev[name]);
    } else if (Array.isArray(name)) {
      return immutable.is(getIn(next, name), getIn(prev, name));
    }

    throw new TypeError('Invalid key: expected Array or string: ' + name);
  };
}

var ImmutablePureComponent =
/*#__PURE__*/
function (_React$Component) {
  _inherits(ImmutablePureComponent, _React$Component);

  function ImmutablePureComponent() {
    _classCallCheck(this, ImmutablePureComponent);

    return _possibleConstructorReturn(this, _getPrototypeOf(ImmutablePureComponent).apply(this, arguments));
  }

  _createClass(ImmutablePureComponent, [{
    key: "shouldComponentUpdate",
    value: function shouldComponentUpdate(nextProps) {
      var nextState = arguments.length > 1 && arguments[1] !== undefined ? arguments[1] : {};
      return !check(this.updateOnProps, this.props, nextProps, 'updateOnProps') || !check(this.updateOnStates, this.state, nextState, 'updateOnStates');
    }
  }]);

  return ImmutablePureComponent;
}(React.Component);

function immutableMemo(Component, updateOnProps) {
  return React.memo(Component, function (prev, next) {
    return check(updateOnProps, prev, next, 'immutableMemo');
  });
}

exports.ImmutablePureComponent = ImmutablePureComponent;
exports.default = ImmutablePureComponent;
exports.immutableMemo = immutableMemo;
