# JSONPath

JSONPath helper for [Zed](https://zed.dev/).

The extension adds a JSON code action that copies the dot-separated key path at the cursor position.

## Usage

Install the LSP binary:

```sh
cargo install --path /Users/Shared/Projects/zed-json-path --force
```

Install the extension in Zed:

1. Run `zed: install dev extension`.
2. Select this repository.
3. Restart Zed so the installed `json-path-lsp` is available on `$PATH`.

Use the code action:

1. Open a JSON file.
2. Put the cursor on a key or value.
3. Run `editor: Toggle Code Actions`.
4. Select `JSONPath: Copy key path`.

For example, placing the cursor on `available` copies:

```text
store.book.available
```

## Settings

The key path separator is configured in Zed's `settings.json` under the `lsp.json-path-lsp.settings` section.

```json
{
  "lsp": {
    "json-path-lsp": {
      "settings": {
        "separator": "."
      }
    }
  }
}
```

The default separator is `.`. For example, set it to `/` to copy paths like:

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
