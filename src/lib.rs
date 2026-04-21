use zed_extension_api as zed;

const DEFAULT_NON_QUOTED_KEY_REGEX: &str = r"^[a-zA-Z$_][a-zA-Z\d$_]*$";
const DEFAULT_PATH_SEPARATOR: &str = ".";
const DEFAULT_PREFIX_SEPARATOR: &str = ":";

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
            return Ok(zed::Command {
                command,
                args: Vec::new(),
                env: CopyJsonPathSettings::for_worktree(worktree).env(),
            });
        }

        Err("install `json-path-lsp` with `cargo install --path /Users/Shared/Projects/zed-json-path --force` and restart Zed".to_string())
    }
}

zed::register_extension!(JsonPathExtension);

pub struct CopyJsonPathSettings {
    pub non_quoted_key_regex: String,
    pub put_file_name_in_path: bool,
    pub prefix_separator: String,
    pub path_separator: String,
}

impl CopyJsonPathSettings {
    pub fn from_env() -> Self {
        Self {
            non_quoted_key_regex: env_string(
                "JSON_PATH_NON_QUOTED_KEY_REGEX",
                DEFAULT_NON_QUOTED_KEY_REGEX,
            ),
            put_file_name_in_path: std::env::var("JSON_PATH_PUT_FILE_NAME_IN_PATH")
                .ok()
                .as_deref()
                == Some("true"),
            prefix_separator: env_string("JSON_PATH_PREFIX_SEPARATOR", DEFAULT_PREFIX_SEPARATOR),
            path_separator: env_string("JSON_PATH_PATH_SEPARATOR", DEFAULT_PATH_SEPARATOR),
        }
    }

    fn for_worktree(worktree: &zed::Worktree) -> Self {
        let settings = zed::settings::LspSettings::for_worktree("json-path-lsp", worktree)
            .ok()
            .and_then(|settings| settings.settings);

        Self {
            non_quoted_key_regex: string_setting(
                settings.as_ref(),
                "nonQuotedKeyRegex",
                DEFAULT_NON_QUOTED_KEY_REGEX,
            ),
            put_file_name_in_path: bool_setting(settings.as_ref(), "putFileNameInPath", false),
            prefix_separator: string_setting(
                settings.as_ref(),
                "prefixSeparator",
                DEFAULT_PREFIX_SEPARATOR,
            ),
            path_separator: string_setting(
                settings.as_ref(),
                "pathSeparator",
                DEFAULT_PATH_SEPARATOR,
            ),
        }
    }

    fn env(&self) -> Vec<(String, String)> {
        vec![
            (
                "JSON_PATH_NON_QUOTED_KEY_REGEX".to_string(),
                self.non_quoted_key_regex.clone(),
            ),
            (
                "JSON_PATH_PUT_FILE_NAME_IN_PATH".to_string(),
                self.put_file_name_in_path.to_string(),
            ),
            (
                "JSON_PATH_PREFIX_SEPARATOR".to_string(),
                self.prefix_separator.clone(),
            ),
            (
                "JSON_PATH_PATH_SEPARATOR".to_string(),
                self.path_separator.clone(),
            ),
        ]
    }
}

pub fn json_key_path_report(
    file_path: &str,
    source: &str,
    row: usize,
    column: usize,
    settings: &CopyJsonPathSettings,
) -> Result<String, String> {
    zed::serde_json::from_str::<zed::serde_json::Value>(source)
        .map_err(|error| format!("failed to parse `{file_path}` as JSON: {error}"))?;

    let offset = byte_offset_for_position(source, row, column)?;
    let path = json_key_path_at_offset(source, offset)?;
    format_json_path(file_path, &path, settings)
}

fn env_string(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn string_setting(settings: Option<&zed::serde_json::Value>, key: &str, default: &str) -> String {
    settings
        .and_then(|settings| settings.get(key))
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .unwrap_or(default)
        .to_string()
}

fn bool_setting(settings: Option<&zed::serde_json::Value>, key: &str, default: bool) -> bool {
    settings
        .and_then(|settings| settings.get(key))
        .and_then(|value| value.as_bool())
        .unwrap_or(default)
}

fn format_json_path(
    file_path: &str,
    path: &[PathSegment],
    settings: &CopyJsonPathSettings,
) -> Result<String, String> {
    let regex = regex::Regex::new(&settings.non_quoted_key_regex)
        .map_err(|error| format!("invalid nonQuotedKeyRegex: {error}"))?;
    let mut path = format_path_segments(path, &settings.path_separator, &regex);

    if settings.put_file_name_in_path {
        let file_name = file_name(file_path).unwrap_or(file_path);

        if path.is_empty() || path == "$" {
            path = file_name.to_string();
        } else {
            path = format!("{file_name}{}{path}", settings.prefix_separator);
        }
    }

    Ok(path)
}

fn format_path_segments(
    path: &[PathSegment],
    path_separator: &str,
    non_quoted_key_regex: &regex::Regex,
) -> String {
    let mut output = String::new();

    for segment in path {
        match segment {
            PathSegment::Key(key) => {
                if output.is_empty() {
                    output.push_str(&format_key_segment(key, non_quoted_key_regex));
                } else if non_quoted_key_regex.is_match(key) {
                    output.push_str(path_separator);
                    output.push_str(key);
                } else {
                    output.push_str(&format_quoted_key(key));
                }
            }
            PathSegment::Index(index) => {
                output.push_str(&format!("[{index}]"));
            }
        }
    }

    if output.is_empty() {
        "$".to_string()
    } else {
        output
    }
}

fn format_key_segment(key: &str, non_quoted_key_regex: &regex::Regex) -> String {
    if non_quoted_key_regex.is_match(key) {
        key.to_string()
    } else {
        format_quoted_key(key)
    }
}

fn format_quoted_key(key: &str) -> String {
    format!(
        "[{}]",
        zed::serde_json::to_string(key).expect("serializing a key cannot fail")
    )
}

fn file_name(file_path: &str) -> Option<&str> {
    let path = file_path.trim_end_matches('/');

    path.rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
}

#[derive(Clone)]
enum PathSegment {
    Key(String),
    Index(usize),
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

fn json_key_path_at_offset(source: &str, offset: usize) -> Result<Vec<PathSegment>, String> {
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
    best_path: Vec<PathSegment>,
}

impl CursorPathParser<'_> {
    fn parse_value(&mut self, path: &mut Vec<PathSegment>) -> Result<(usize, usize), String> {
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

    fn parse_object(&mut self, path: &mut Vec<PathSegment>) -> Result<usize, String> {
        self.expect_byte(b'{')?;

        loop {
            self.skip_whitespace();

            if self.consume_byte(b'}') {
                return Ok(self.position);
            }

            let (key, key_end) = self.parse_string()?;
            path.push(PathSegment::Key(key));
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

    fn parse_array(&mut self, path: &mut Vec<PathSegment>) -> Result<usize, String> {
        self.expect_byte(b'[')?;
        let mut index = 0;

        loop {
            self.skip_whitespace();

            if self.consume_byte(b']') {
                return Ok(self.position);
            }

            path.push(PathSegment::Index(index));
            self.parse_value(path)?;
            path.pop();
            self.skip_whitespace();

            if self.consume_byte(b',') {
                index += 1;
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

    fn consider(&mut self, start: usize, end: usize, path: &[PathSegment]) {
        if start <= self.offset && self.offset <= end && path.len() >= self.best_path.len() {
            self.best_path = path.to_vec();
        }
    }
}
