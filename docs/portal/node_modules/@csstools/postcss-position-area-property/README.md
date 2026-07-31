# PostCSS Position Area Property [<img src="https://postcss.github.io/postcss/logo.svg" alt="PostCSS Logo" width="90" height="90" align="right">][PostCSS]

`npm install @csstools/postcss-position-area-property --save-dev`

[PostCSS Position Area Property] lets you fallback `position-area` to the alternate name `inset-area` following the [CSS Specification].

```css
.foo {
	position-area: start;
}

/* becomes */

.foo {
	inset-area: start;
	position-area: start;
}
```

## Usage

Add [PostCSS Position Area Property] to your project:

```bash
npm install postcss @csstools/postcss-position-area-property --save-dev
```

Use it as a [PostCSS] plugin:

```js
const postcss = require('postcss');
const postcssPositionAreaProperty = require('@csstools/postcss-position-area-property');

postcss([
	postcssPositionAreaProperty(/* pluginOptions */)
]).process(YOUR_CSS /*, processOptions */);
```



[cli-url]: https://github.com/csstools/postcss-plugins/actions/workflows/test.yml?query=workflow/test
[css-url]: https://cssdb.org/#position-area
[discord]: https://discord.gg/bUadyRwkJS
[npm-url]: https://www.npmjs.com/package/@csstools/postcss-position-area-property

[PostCSS]: https://github.com/postcss/postcss
[PostCSS Position Area Property]: https://github.com/csstools/postcss-plugins/tree/main/plugins/postcss-position-area-property
[CSS Specification]: https://drafts.csswg.org/css-anchor-position/#position-area
