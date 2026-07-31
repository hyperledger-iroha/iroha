"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-7");
/* eslint-disable class-methods-use-this */

/**
 * URI: https://json-schema.org/draft/2019-09/draft-handrews-json-schema-hyperschema-02#rfc.section.6
 * @public
 */

class LinkDescription extends _apidomNsJsonSchemaDraft.LinkDescriptionElement {
  /**
   *  Link Target Attributes.
   *
   *  URI: https://json-schema.org/draft/2019-09/draft-handrews-json-schema-hyperschema-02#rfc.section.6.5
   */
  get targetSchema() {
    return this.get('targetSchema');
  }
  set targetSchema(targetSchema) {
    this.set('targetSchema', targetSchema);
  }

  /**
   *  Link Input.
   *
   *  URI: https://json-schema.org/draft/2019-09/draft-handrews-json-schema-hyperschema-02#input
   */
  get hrefSchema() {
    return this.get('hrefSchema');
  }
  set hrefSchema(hrefSchema) {
    this.set('hrefSchema', hrefSchema);
  }
  get headerSchema() {
    return this.get('headerSchema');
  }
  set headerSchema(headerSchema) {
    this.set('headerSchema', headerSchema);
  }
  get submissionSchema() {
    return this.get('submissionSchema');
  }
  set submissionSchema(submissionSchema) {
    this.set('submissionSchema', submissionSchema);
  }
}
var _default = exports.default = LinkDescription;