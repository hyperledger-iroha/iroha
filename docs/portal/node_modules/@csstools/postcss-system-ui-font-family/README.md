# PostCSS System UI Font Family [<img src="https://postcss.github.io/postcss/logo.svg" alt="PostCSS Logo" width="90" height="90" align="right">][PostCSS]

`npm install @csstools/postcss-system-ui-font-family --save-dev`

[PostCSS System UI Font Family] lets you use the `system-ui` keyword following the [CSS Fonts 4 Specification].

```css
.foo {
	font: italic bold 12px/30px system-ui;
	font-family: system-ui;
	--font-family: system-ui;
}

/* becomes */

.foo {
	font: italic bold 12px/30px system-ui;
	font-family: system-ui,-apple-system,Segoe UI,Roboto,Ubuntu,Cantarell,Noto Sans,sans-serif;
	--font-family: system-ui,-apple-system,Segoe UI,Roboto,Ubuntu,Cantarell,Noto Sans,sans-serif;
}
```

## Usage

Add [PostCSS System UI Font Family] to your project:

```bash
npm install postcss @csstools/postcss-system-ui-font-family --save-dev
```

Use it as a [PostCSS] plugin:

```js
const postcss = require('postcss');
const postcssSystemUIFontFamily = require('@csstools/postcss-system-ui-font-family');

postcss([
	postcssSystemUIFontFamily(/* pluginOptions */)
]).process(YOUR_CSS /*, processOptions */);
```



## Options

### preserve

The `preserve` option determines whether the original notation
is preserved. By default, it is preserved.

```js
postcssSystemUIFontFamily({ preserve: false })
```

```css
.foo {
	font: italic bold 12px/30px system-ui;
	font-family: system-ui;
	--font-family: system-ui;
}

/* becomes */

.foo {
	font: italic bold 12px/30px system-ui;
	font-family: -apple-system,Segoe UI,Roboto,Ubuntu,Cantarell,Noto Sans,sans-serif;
	--font-family: -apple-system,Segoe UI,Roboto,Ubuntu,Cantarell,Noto Sans,sans-serif;
}
```

[cli-url]: https://github.com/csstools/postcss-plugins/actions/workflows/test.yml?query=workflow/test
[css-url]: https://cssdb.org/#system-ui-font-family
[discord]: https://discord.gg/bUadyRwkJS
[npm-url]: https://www.npmjs.com/package/@csstools/postcss-system-ui-font-family

[PostCSS]: https://github.com/postcss/postcss
[PostCSS System UI Font Family]: https://github.com/csstools/postcss-plugins/tree/main/plugins/postcss-system-ui-font-family
[CSS Fonts 4 Specification]: https://drafts.csswg.org/css-fonts-4/#system-ui-def
