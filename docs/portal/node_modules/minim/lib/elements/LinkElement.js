const Element = require('../primitives/Element');

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
module.exports = class LinkElement extends Element {
  constructor(content, meta, attributes) {
    super(content || [], meta, attributes);
    this.element = 'link';
  }

  /**
   * The relation identifier for the link, as defined in RFC 5988.
   * @type StringElement
   */
  get relation() {
    return this.attributes.get('relation');
  }

  set relation(relation) {
    this.attributes.set('relation', relation);
  }

  /**
   * The URI for the given link.
   * @type StringElement
   */
  get href() {
    return this.attributes.get('href');
  }

  set href(href) {
    this.attributes.set('href', href);
  }
};
