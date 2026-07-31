const Namespace = require('./Namespace');
const elements = require('./elements');

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
