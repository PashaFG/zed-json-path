# JSONPath

JSONPath support for [Zed](https://zed.dev/).

This repository is a starter template for a Zed extension. It contains the required extension manifest, a minimal Rust extension entrypoint, CI, and a license file.

## Local development

Install Rust through `rustup`, then add the WASI target used by Zed extensions:

```sh
rustup target add wasm32-wasip2
```

Check the extension:

```sh
cargo check --target wasm32-wasip2
```

To test the extension in Zed:

1. Open the command palette.
2. Run `zed: extensions`.
3. Click `Install Dev Extension`.
4. Select this repository directory.

For debug output, start Zed from a terminal:

```sh
zed --foreground
```

## Publishing

To publish this extension, open a pull request to [`zed-industries/extensions`](https://github.com/zed-industries/extensions).

Add this repository as a submodule:

```sh
git submodule add https://github.com/PashaFG/zed-json-path.git extensions/json-path
```

Then add an entry to `extensions.toml`:

```toml
[json-path]
submodule = "extensions/json-path"
version = "0.0.1"
```

Run `pnpm sort-extensions` in the `zed-industries/extensions` repository before submitting the pull request.

## Next steps

To turn this template into actual JSONPath language support, add:

- a Tree-sitter JSONPath grammar reference in `extension.toml`;
- `languages/jsonpath/config.toml`;
- Tree-sitter queries such as `highlights.scm`, `brackets.scm`, and `outline.scm`;
- optional snippets or a language server if needed.
