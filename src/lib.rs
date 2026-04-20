use zed_extension_api as zed;

struct JsonPathExtension;

impl zed::Extension for JsonPathExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        if language_server_id.as_ref() != "json-path-lsp" {
            return Err(format!("unknown language server: {language_server_id}"));
        }

        if let Some(command) = worktree.which("json-path-lsp") {
            let separator = separator_setting(worktree);

            return Ok(zed::Command {
                command,
                args: Vec::new(),
                env: vec![("JSON_PATH_SEPARATOR".to_string(), separator)],
            });
        }

        Err("install `json-path-lsp` with `cargo install --path /Users/Shared/Projects/zed-json-path --force` and restart Zed".to_string())
    }
}

zed::register_extension!(JsonPathExtension);

fn separator_setting(worktree: &zed::Worktree) -> String {
    zed::settings::LspSettings::for_worktree("json-path-lsp", worktree)
        .ok()
        .and_then(|settings| settings.settings)
        .and_then(|settings| {
            settings
                .get("separator")
                .and_then(|separator| separator.as_str())
                .map(str::to_string)
        })
        .filter(|separator| !separator.is_empty())
        .unwrap_or_else(|| ".".to_string())
}

pub fn json_key_path_report(
    file_path: &str,
    source: &str,
    row: usize,
    column: usize,
    separator: &str,
) -> Result<String, String> {
    zed::serde_json::from_str::<zed::serde_json::Value>(source)
        .map_err(|error| format!("failed to parse `{file_path}` as JSON: {error}"))?;

    let offset = byte_offset_for_position(source, row, column)?;
    json_key_path_at_offset(source, offset).map(|path| {
        if path.is_empty() {
            "$".to_string()
        } else {
            path.join(separator)
        }
    })
}

fn byte_offset_for_position(source: &str, row: usize, column: usize) -> Result<usize, String> {
    let input_row = row;
    let input_column = column;
    let row = input_row
        .checked_sub(1)
        .ok_or_else(|| "cursor row must be 1-based".to_string())?;
    let column = input_column
        .checked_sub(1)
        .ok_or_else(|| "cursor column must be 1-based".to_string())?;
    let mut current_row = 0;
    let mut current_column = 0;

    for (offset, character) in source.char_indices() {
        if current_row == row && current_column == column {
            return Ok(offset);
        }

        if character == '\n' {
            current_row += 1;
            current_column = 0;
        } else {
            current_column += 1;
        }
    }

    if current_row == row && current_column == column {
        return Ok(source.len());
    }

    Err(format!(
        "cursor position {input_row}:{input_column} is outside the file"
    ))
}

fn json_key_path_at_offset(source: &str, offset: usize) -> Result<Vec<String>, String> {
    let mut parser = CursorPathParser {
        source,
        bytes: source.as_bytes(),
        offset,
        position: 0,
        best_path: Vec::new(),
    };

    let mut root_path = Vec::new();
    parser.parse_value(&mut root_path)?;
    Ok(parser.best_path)
}

struct CursorPathParser<'a> {
    source: &'a str,
    bytes: &'a [u8],
    offset: usize,
    position: usize,
    best_path: Vec<String>,
}

impl CursorPathParser<'_> {
    fn parse_value(&mut self, path: &mut Vec<String>) -> Result<(usize, usize), String> {
        self.skip_whitespace();
        let start = self.position;

        let end = match self.current_byte() {
            Some(b'{') => self.parse_object(path)?,
            Some(b'[') => self.parse_array(path)?,
            Some(b'"') => {
                let (_, span) = self.parse_string()?;
                span.1
            }
            Some(b'-' | b'0'..=b'9') => self.parse_number(),
            Some(b't') => self.parse_literal("true")?,
            Some(b'f') => self.parse_literal("false")?,
            Some(b'n') => self.parse_literal("null")?,
            Some(byte) => return Err(format!("unexpected byte `{}` at {start}", byte as char)),
            None => return Err("unexpected end of JSON".to_string()),
        };

        self.consider(start, end, path);
        Ok((start, end))
    }

    fn parse_object(&mut self, path: &mut Vec<String>) -> Result<usize, String> {
        self.expect_byte(b'{')?;

        loop {
            self.skip_whitespace();

            if self.consume_byte(b'}') {
                return Ok(self.position);
            }

            let (key, key_end) = self.parse_string()?;
            path.push(key);
            self.consider(key_end.0, key_end.1, path);

            self.skip_whitespace();
            self.expect_byte(b':')?;
            self.parse_value(path)?;
            path.pop();

            self.skip_whitespace();

            if self.consume_byte(b',') {
                continue;
            }

            self.expect_byte(b'}')?;
            return Ok(self.position);
        }
    }

    fn parse_array(&mut self, path: &mut Vec<String>) -> Result<usize, String> {
        self.expect_byte(b'[')?;

        loop {
            self.skip_whitespace();

            if self.consume_byte(b']') {
                return Ok(self.position);
            }

            self.parse_value(path)?;
            self.skip_whitespace();

            if self.consume_byte(b',') {
                continue;
            }

            self.expect_byte(b']')?;
            return Ok(self.position);
        }
    }

    fn parse_string(&mut self) -> Result<(String, (usize, usize)), String> {
        let start = self.position;
        self.expect_byte(b'"')?;

        while let Some(byte) = self.current_byte() {
            self.position += 1;

            match byte {
                b'\\' => {
                    if self.current_byte().is_some() {
                        self.position += 1;
                    }
                }
                b'"' => {
                    let end = self.position;
                    let value = zed::serde_json::from_str(&self.source[start..end])
                        .map_err(|error| format!("failed to parse JSON string: {error}"))?;
                    return Ok((value, (start, end)));
                }
                _ => {}
            }
        }

        Err("unterminated JSON string".to_string())
    }

    fn parse_number(&mut self) -> usize {
        while matches!(
            self.current_byte(),
            Some(b'-' | b'+' | b'.' | b'0'..=b'9' | b'e' | b'E')
        ) {
            self.position += 1;
        }

        self.position
    }

    fn parse_literal(&mut self, literal: &str) -> Result<usize, String> {
        if self.source[self.position..].starts_with(literal) {
            self.position += literal.len();
            Ok(self.position)
        } else {
            Err(format!("expected `{literal}` at {}", self.position))
        }
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.current_byte(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.position += 1;
        }
    }

    fn current_byte(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }

    fn consume_byte(&mut self, byte: u8) -> bool {
        if self.current_byte() == Some(byte) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn expect_byte(&mut self, byte: u8) -> Result<(), String> {
        if self.consume_byte(byte) {
            Ok(())
        } else {
            Err(format!("expected `{}` at {}", byte as char, self.position))
        }
    }

    fn consider(&mut self, start: usize, end: usize, path: &[String]) {
        if start <= self.offset && self.offset <= end && path.len() >= self.best_path.len() {
            self.best_path = path.to_vec();
        }
    }
}
