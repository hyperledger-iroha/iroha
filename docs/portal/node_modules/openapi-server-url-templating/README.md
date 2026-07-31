# openapi-server-url-templating

[![npmversion](https://badge.fury.io/js/openapi-server-url-templating.svg)](https://www.npmjs.com/package/openapi-server-url-templating)
[![npm](https://img.shields.io/npm/dm/openapi-server-url-templating.svg)](https://www.npmjs.com/package/openapi-server-url-templating)
[![Test workflow](https://github.com/swaggerexpert/openapi-server-url-templating/actions/workflows/test.yml/badge.svg)](https://github.com/swaggerexpert/openapi-server-url-templating/actions)
[![Dependabot enabled](https://img.shields.io/badge/Dependabot-enabled-blue.svg)](https://dependabot.com/)
[![try on RunKit](https://img.shields.io/badge/try%20on-RunKit-brightgreen.svg?style=flat)](https://npm.runkit.com/openapi-server-url-templating)
[![Tidelift](https://tidelift.com/badges/package/npm/openapi-server-url-templating)](https://tidelift.com/subscription/pkg/npm-openapi-server-url-templating?utm_source=npm-openapi-server-url-templating&utm_medium=referral&utm_campaign=readme)

[Server URL Templating](https://spec.openapis.org/oas/v3.1.1.html#server-object) supports [Server Variables](https://spec.openapis.org/oas/v3.1.1.html#server-variable-object). Variable substitutions will be made when a variable is named in `{`brackets`}`.
This mechanism is used by [Server Object](https://spec.openapis.org/oas/v3.1.1.html#server-object)
of [OpenAPI specification](https://spec.openapis.org/).

`openapi-server-url-templating` is a **parser**, **validator** and **substitution mechanism** for OpenAPI Server URL Templating,
which played a [foundational role](https://github.com/OAI/OpenAPI-Specification/pull/4264) in defining the official ANBF grammar for Server URL Templating.

It supports Server Object URL Templating defined in following OpenAPI specification versions:

- [OpenAPI 3.0.0](https://spec.openapis.org/oas/v3.0.0.html)
- [OpenAPI 3.0.1](https://spec.openapis.org/oas/v3.0.1.html)
- [OpenAPI 3.0.2](https://spec.openapis.org/oas/v3.0.2.html)
- [OpenAPI 3.0.3](https://spec.openapis.org/oas/v3.0.3.html)
- [OpenAPI 3.0.4](https://spec.openapis.org/oas/v3.0.4.html)
- [OpenAPI 3.1.0](https://spec.openapis.org/oas/v3.1.0.html)
- [OpenAPI 3.1.1](https://spec.openapis.org/oas/v3.1.1.html)

<table>
  <tr>
    <td align="right" valign="middle">
        <img src="https://cdn2.hubspot.net/hubfs/4008838/website/logos/logos_for_download/Tidelift_primary-shorthand-logo.png" alt="Tidelift" width="60" />
      </td>
      <td valign="middle">
        <a href="https://tidelift.com/subscription/pkg/npm-openapi-server-url-templating?utm_source=npm-openapi-server-url-templating&utm_medium=referral&utm_campaign=readme">
            Get professionally supported openapi-server-url-templating with Tidelift Subscription.
        </a>
      </td>
  </tr>
</table>

## Table of Contents

- [Getting started](#getting-started)
  - [Installation](#installation)
  - [Usage](#usage)
    - [Parsing](#parsing)
    - [Validation](#validation)
    - [Substitution](#substitution)
    - [Grammar](#grammar)
- [More about OpenAPI Server URL Templating](#more-about-openapi-server-url-templating)
- [License](#license)

## Getting started

### Installation

You can install `openapi-server-url-templating` using `npm`:

```sh
 $ npm install openapi-server-url-templating
```

### Usage

`openapi-server-url-templating` currently supports **parsing**, **validation** and **substitution**.
Both parser and validator are based on a superset of [ABNF](https://www.rfc-editor.org/rfc/rfc5234) ([SABNF](https://github.com/ldthomas/apg-js2/blob/master/SABNF.md))
and use [apg-lite](https://github.com/ldthomas/apg-lite) parser generator.

#### Parsing

Parsing a Server URL Templating is as simple as importing the **parse** function
and calling it.

```js
import { parse } from 'openapi-server-url-templating';

const parseResult = parse('https://{username}.gigantic-server.com:{port}/{basePath}');
parseResult.result.success; // => true
```

**parseResult** variable has the following shape:

```
{
  result: {
    success: true,
    state: 101,
    stateName: 'MATCH',
    length: 56,
    matched: 56,
    maxMatched: 56,
    maxTreeDepth: 12,
    nodeHits: 758
  },
  ast: fnast {
    callbacks: [
      'server-url-template': [Function: serverUrlTemplate],
      'server-variable': [Function: serverVariable],
      'server-variable-name': [Function: serverVariableName],
      literals: [Function: literals]
    ],
    init: [Function (anonymous)],
    ruleDefined: [Function (anonymous)],
    udtDefined: [Function (anonymous)],
    down: [Function (anonymous)],
    up: [Function (anonymous)],
    translate: [Function (anonymous)],
    setLength: [Function (anonymous)],
    getLength: [Function (anonymous)],
    toXml: [Function (anonymous)]
  }
}
```

###### Interpreting AST as list of entries

```js
import { parse } from 'openapi-server-url-templating';

const parseResult = parse('https://{username}.gigantic-server.com:{port}/{basePath}');
const parts = [];

parseResult.ast.translate(parts);
```

After running the above code, **parts** variable has the following shape:

```js
[
  [
    'server-url-template',
    'https://{username}.gigantic-server.com:{port}/{basePath}'
  ],
  [ 'literals', 'https://' ],
  [ 'server-variable', '{username}' ],
  [ 'server-variable-name', 'username' ],
  [ 'literals', '.gigantic-server.com:' ],
  [ 'server-variable', '{port}' ],
  [ 'server-variable-name', 'port' ],
  [ 'literals', '/' ],
  [ 'server-variable', '{basePath}' ],
  [ 'server-variable-name', 'basePath' ]
]
```

###### Interpreting AST as XML

```js
import { parse } from 'openapi-server-url-templating';

const parseResult = parse('https://{username}.gigantic-server.com:{port}/{basePath}');
const xml = parseResult.ast.toXml();
```

After running the above code, **xml** variable has the following content:

```xml
<?xml version="1.0" encoding="utf-8"?>
<root nodes="10" characters="56">
  <!-- input string -->
  https://{username}.gigantic-server.com:{port}/{basePath}
  <node name="server-url-template" index="0" length="56">
    https://{username}.gigantic-server.com:{port}/{basePath}
    <node name="literals" index="0" length="8">
      https://
    </node><!-- name="literals" -->
    <node name="server-variable" index="8" length="10">
      {username}
      <node name="server-variable-name" index="9" length="8">
        username
      </node><!-- name="server-variable-name" -->
    </node><!-- name="server-variable" -->
    <node name="literals" index="18" length="21">
      .gigantic-server.com:
    </node><!-- name="literals" -->
    <node name="server-variable" index="39" length="6">
      {port}
      <node name="server-variable-name" index="40" length="4">
        port
      </node><!-- name="server-variable-name" -->
    </node><!-- name="server-variable" -->
    <node name="literals" index="45" length="1">
      /
    </node><!-- name="literals" -->
    <node name="server-variable" index="46" length="10">
      {basePath}
      <node name="server-variable-name" index="47" length="8">
        basePath
      </node><!-- name="server-variable-name" -->
    </node><!-- name="server-variable" -->
  </node><!-- name="server-url-template" -->
</root>
```

> NOTE: AST can also be traversed in classical way using [depth first traversal](https://www.tutorialspoint.com/data_structures_algorithms/depth_first_traversal.htm). For more information about this option please refer to [apg-js](https://github.com/ldthomas/apg-js) and [apg-js-examples](https://github.com/ldthomas/apg-js-examples).

#### Validation

Validating a Server URL Templating is as simple as importing the **test** function and calling it.


```js
import { test } from 'openapi-server-url-templating';

test('https://{username}.gigantic-server.com:{port}/{basePath}'); // => true
test('https://gigantic-server.com/base-path'); // => true
test('https://gigantic-server.com/base-path', { strict: true }); // => false (doesn't contain any server-variable)
```

#### Substitution

Performing Server URL template substitution is as simple as importing the **substitute** function and calling it.

```js
import { substitute } from 'openapi-server-url-templating';

subtitute('https://{username}.gigantic-server.com', { username: 'char0n' }); // => "https://char0n.gigantic-server.com"
```

Substituted Server URL Templating is automatically encoded using [encodeURIComponent](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/encodeURIComponent) function.
It is possible to provide a custom encoder.

```js
import { substitute } from 'openapi-server-url-templating';

substitute('https://{username}.gigantic-server.com', { username: '/?#' }, {
  encoder: (serverVariable) => serverVariable, // no encoding
}); // => "https:///?#.gigantic-server.com"
```

#### Grammar

New grammar instance can be created in following way:

```js
import { Grammar } from 'openapi-server-url-templating';

const grammar = new Grammar();
```

To obtain original ABNF (SABNF) grammar as a string:

```js
import { Grammar } from 'openapi-server-url-templating';

const grammar = new Grammar();

grammar.toString();
// or
String(grammar);
```

## More about OpenAPI Server URL Templating

The Server URL Templating is defined by the following [ABNF](https://tools.ietf.org/html/rfc5234) syntax

```abnf
; OpenAPI Server URL templating ABNF syntax
server-url-template    = 1*( literals / server-variable ) ; variant of https://www.rfc-editor.org/rfc/rfc6570#section-2
server-variable        = "{" server-variable-name "}"
server-variable-name   = 1*( %x00-7A / %x7C / %x7E-10FFFF ) ; every UTF8 character except { and } (from OpenAPI)

; https://www.rfc-editor.org/rfc/rfc6570#section-2.1
; https://www.rfc-editor.org/errata/eid6937
literals               = 1*( %x21 / %x23-24 / %x26-3B / %x3D / %x3F-5B
                       / %x5D / %x5F / %x61-7A / %x7E / ucschar / iprivate
                       / pct-encoded)
                            ; any Unicode character except: CTL, SP,
                            ;  DQUOTE, "%" (aside from pct-encoded),
                            ;  "<", ">", "\", "^", "`", "{", "|", "}"

; https://www.rfc-editor.org/rfc/rfc6570#section-1.5
DIGIT          =  %x30-39             ; 0-9
HEXDIG         =  DIGIT / "A" / "B" / "C" / "D" / "E" / "F" ; case-insensitive

pct-encoded    =  "%" HEXDIG HEXDIG

ucschar        =  %xA0-D7FF / %xF900-FDCF / %xFDF0-FFEF
               /  %x10000-1FFFD / %x20000-2FFFD / %x30000-3FFFD
               /  %x40000-4FFFD / %x50000-5FFFD / %x60000-6FFFD
               /  %x70000-7FFFD / %x80000-8FFFD / %x90000-9FFFD
               /  %xA0000-AFFFD / %xB0000-BFFFD / %xC0000-CFFFD
               /  %xD0000-DFFFD / %xE1000-EFFFD

iprivate       =  %xE000-F8FF / %xF0000-FFFFD / %x100000-10FFFD
```

## License

`openapi-server-url-templating` is licensed under [Apache 2.0 license](https://github.com/swaggerexpert/openapi-server-url-templating/blob/main/LICENSE).
`openapi-server-url-templating` comes with an explicit [NOTICE](https://github.com/swaggerexpert/openapi-server-url-templating/blob/main/NOTICE) file
containing additional legal notices and information.
