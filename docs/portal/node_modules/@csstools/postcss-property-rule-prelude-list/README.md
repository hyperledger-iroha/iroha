# PostCSS Property Rule Prelude List [<img src="https://postcss.github.io/postcss/logo.svg" alt="PostCSS Logo" width="90" height="90" align="right">][PostCSS]

`npm install @csstools/postcss-property-rule-prelude-list --save-dev`

[PostCSS Property Rule Prelude List] lets you declare a list of custom properties in a single `@property` rule following the [CSS Specification].

```css
@property --color-a, --color-b {
	inherits: true;
	initial-value: black;
	syntax: "<color>";
}

/* becomes */

@property --color-a {
	inherits: true;
	initial-value: black;
	syntax: "<color>";
}
@property --color-b {
	inherits: true;
	initial-value: black;
	syntax: "<color>";
}
```

## Usage

Add [PostCSS Property Rule Prelude List] to your project:

```bash
npm install postcss @csstools/postcss-property-rule-prelude-list --save-dev
```

Use it as a [PostCSS] plugin:

```js
const postcss = require('postcss');
const postcssPropertyRulePreludeList = require('@csstools/postcss-property-rule-prelude-list');

postcss([
	postcssPropertyRulePreludeList(/* pluginOptions */)
]).process(YOUR_CSS /*, processOptions */);
```



[cli-url]: https://github.com/csstools/postcss-plugins/actions/workflows/test.yml?query=workflow/test
[css-url]: https://cssdb.org/#property-rule-prelude-list
[discord]: https://discord.gg/bUadyRwkJS
[npm-url]: https://www.npmjs.com/package/@csstools/postcss-property-rule-prelude-list

[PostCSS]: https://github.com/postcss/postcss
[PostCSS Property Rule Prelude List]: https://github.com/csstools/postcss-plugins/tree/main/plugins/postcss-property-rule-prelude-list
[CSS Specification]: https://github.com/w3c/csswg-drafts/issues/7523#issuecomment-3683970305
