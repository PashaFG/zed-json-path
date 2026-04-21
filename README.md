# JSONPath

JSONPath helper for [Zed](https://zed.dev/).

The extension adds a JSON code action that copies the key path at the cursor position.
It is a Zed alternative to the VS Code extension [Copy JSON Path](https://marketplace.visualstudio.com/items?itemName=nidu.copy-json-path).

## Usage

Install the extension in Zed:

1. Run `zed: install dev extension`.
2. Select this repository.

Use the code action:

1. Open a JSON file.
2. Put the cursor on a key or value.
3. Run `editor: Toggle Code Actions`.
4. Select `JSONPath: Copy key path`.

For example, placing the cursor on `k` in this JSON:

```json
{
  "a": ["q", { "k": 1 }, 1]
}
```

copies:

```text
a[1].k
```

## Settings

The copied path is configured in Zed's `settings.json` under the `lsp.json-path-lsp.settings` section.

```json
{
  "lsp": {
    "json-path-lsp": {
      "settings": {
        "nonQuotedKeyRegex": "^[a-zA-Z$_][a-zA-Z\\d$_]*$",
        "putFileNameInPath": false,
        "prefixSeparator": ":",
        "pathSeparator": "."
      }
    }
  }
}
```

Available settings:

- `nonQuotedKeyRegex`: regex that tests whether a key can be copied without quotes. Default is `^[a-zA-Z$_][a-zA-Z\\d$_]*$`.
- `putFileNameInPath`: include the file name before the copied path. Default is `false`.
- `prefixSeparator`: separator between the file name and the path when `putFileNameInPath` is `true`. Default is `:`.
- `pathSeparator`: separator between path parts. Default is `.`.

For example, set `pathSeparator` to `/` to copy paths like:

```text
store/book/available
```

## Development

Install the WASI target used by Zed extensions:

```sh
rustup target add wasm32-wasip2
```

Run checks manually:

```sh
cargo fmt --check
cargo check --target wasm32-wasip2
cargo check --bin json-path-lsp
```

Create a release:

```sh
git tag v0.0.2
git push origin v0.0.2
```

The release workflow builds `json-path-lsp` and uploads the required assets to the GitHub Release.

For local development before a GitHub Release exists, install the LSP binary manually:

```sh
cargo install --path . --force
```

When `json-path-lsp` is not available on `$PATH`, the extension downloads it from the latest GitHub Release.
Release assets must be uncompressed executable binaries with these names:

```text
json-path-lsp-macos-aarch64
json-path-lsp-macos-x86_64
json-path-lsp-linux-aarch64
json-path-lsp-linux-x86_64
json-path-lsp-windows-aarch64.exe
json-path-lsp-windows-x86_64.exe
```
