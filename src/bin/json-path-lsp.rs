use std::{
    collections::HashMap,
    io::{self, BufRead, BufReader, Read, Write},
};

#[cfg(any(target_os = "macos", target_os = "windows", unix))]
use std::process::{Command, Stdio};

use zed_extension_api::serde_json::{json, Value};

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut server = JsonPathLsp::default();
    server.run(BufReader::new(stdin.lock()), io::stdout().lock())
}

#[derive(Default)]
struct JsonPathLsp {
    documents: HashMap<String, String>,
    shutdown_requested: bool,
}

impl JsonPathLsp {
    fn run<R, W>(&mut self, mut reader: BufReader<R>, mut writer: W) -> io::Result<()>
    where
        R: Read,
        W: Write,
    {
        while let Some(message) = read_lsp_message(&mut reader)? {
            let Some(method) = message.get("method").and_then(Value::as_str) else {
                continue;
            };

            match method {
                "initialize" => {
                    self.respond(
                        &mut writer,
                        message.get("id").cloned(),
                        json!({
                            "capabilities": {
                                "textDocumentSync": {
                                    "openClose": true,
                                    "change": 1
                                },
                                "codeActionProvider": true,
                                "executeCommandProvider": {
                                    "commands": ["jsonPath.copyKeyPath"]
                                }
                            }
                        }),
                    )?;
                }
                "initialized" => {}
                "shutdown" => {
                    self.shutdown_requested = true;
                    self.respond(&mut writer, message.get("id").cloned(), Value::Null)?;
                }
                "exit" => break,
                "textDocument/didOpen" => self.did_open(&message),
                "textDocument/didChange" => self.did_change(&message),
                "textDocument/didClose" => self.did_close(&message),
                "textDocument/codeAction" => {
                    let actions = self.code_actions(&message);
                    self.respond(&mut writer, message.get("id").cloned(), actions)?;
                }
                "workspace/executeCommand" => {
                    let result = self.execute_command(&message);
                    self.respond(&mut writer, message.get("id").cloned(), result)?;
                }
                _ if message.get("id").is_some() => {
                    self.respond(&mut writer, message.get("id").cloned(), Value::Null)?;
                }
                _ => {}
            }

            if self.shutdown_requested && method == "exit" {
                break;
            }
        }

        Ok(())
    }

    fn respond<W: Write>(
        &self,
        writer: &mut W,
        id: Option<Value>,
        result: Value,
    ) -> io::Result<()> {
        let response = json!({
            "jsonrpc": "2.0",
            "id": id.unwrap_or(Value::Null),
            "result": result
        });
        write_lsp_message(writer, &response)
    }

    fn did_open(&mut self, message: &Value) {
        let Some(document) = message.pointer("/params/textDocument") else {
            return;
        };
        let Some(uri) = document.get("uri").and_then(Value::as_str) else {
            return;
        };
        let Some(text) = document.get("text").and_then(Value::as_str) else {
            return;
        };

        self.documents.insert(uri.to_string(), text.to_string());
    }

    fn did_change(&mut self, message: &Value) {
        let Some(uri) = message
            .pointer("/params/textDocument/uri")
            .and_then(Value::as_str)
        else {
            return;
        };
        let Some(changes) = message
            .pointer("/params/contentChanges")
            .and_then(Value::as_array)
        else {
            return;
        };
        let Some(text) = changes
            .last()
            .and_then(|change| change.get("text"))
            .and_then(Value::as_str)
        else {
            return;
        };

        self.documents.insert(uri.to_string(), text.to_string());
    }

    fn did_close(&mut self, message: &Value) {
        let Some(uri) = message
            .pointer("/params/textDocument/uri")
            .and_then(Value::as_str)
        else {
            return;
        };

        self.documents.remove(uri);
    }

    fn code_actions(&self, message: &Value) -> Value {
        let Some(params) = message.get("params") else {
            return json!([]);
        };
        let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str) else {
            return json!([]);
        };
        let Some(source) = self.documents.get(uri) else {
            return json!([]);
        };
        let Some(start) = params.pointer("/range/start") else {
            return json!([]);
        };
        let Some(line) = start.get("line").and_then(Value::as_u64) else {
            return json!([]);
        };
        let Some(character) = start.get("character").and_then(Value::as_u64) else {
            return json!([]);
        };

        let Ok(key_path) =
            json_path::json_key_path_report(uri, source, line as usize + 1, character as usize + 1)
        else {
            return json!([]);
        };

        json!([
            {
                "title": "JSONPath: Copy key path",
                "kind": "refactor",
                "command": {
                    "title": "JSONPath: Copy key path",
                    "command": "jsonPath.copyKeyPath",
                    "arguments": [key_path]
                }
            }
        ])
    }

    fn execute_command(&self, message: &Value) -> Value {
        let Some(command) = message.pointer("/params/command").and_then(Value::as_str) else {
            return Value::Null;
        };

        if command != "jsonPath.copyKeyPath" {
            return Value::Null;
        }

        let Some(key_path) = message
            .pointer("/params/arguments/0")
            .and_then(Value::as_str)
        else {
            return Value::Null;
        };

        let _ = copy_to_clipboard(key_path);
        Value::Null
    }
}

fn copy_to_clipboard(text: &str) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        return write_to_clipboard_command("pbcopy", &[], text);
    }

    #[cfg(target_os = "windows")]
    {
        return write_to_clipboard_command("clip", &[], text);
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        for (command, args) in [
            ("wl-copy", Vec::<&str>::new()),
            ("xclip", vec!["-selection", "clipboard"]),
            ("xsel", vec!["--clipboard", "--input"]),
        ] {
            if write_to_clipboard_command(command, &args, text).is_ok() {
                return Ok(());
            }
        }

        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no clipboard command found: expected wl-copy, xclip, or xsel",
        ))
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", unix)))]
    {
        let _ = text;

        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "clipboard is not supported on this target",
        ))
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", unix))]
fn write_to_clipboard_command(command: &str, args: &[&str], text: &str) -> io::Result<()> {
    let mut child = Command::new(command)
        .args(args)
        .stdin(Stdio::piped())
        .spawn()?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(text.as_bytes())?;
    }

    let status = child.wait()?;

    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "`{command}` exited with status {status}"
        )))
    }
}

fn read_lsp_message<R: Read>(reader: &mut BufReader<R>) -> io::Result<Option<Value>> {
    let mut content_length = None;

    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;

        if bytes_read == 0 {
            return Ok(None);
        }

        let line = line.trim_end_matches(['\r', '\n']);

        if line.is_empty() {
            break;
        }

        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse::<usize>().ok();
            }
        }
    }

    let Some(content_length) = content_length else {
        return Ok(None);
    };
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body)?;

    let message = zed_extension_api::serde_json::from_slice(&body)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(Some(message))
}

fn write_lsp_message<W: Write>(writer: &mut W, message: &Value) -> io::Result<()> {
    let body = zed_extension_api::serde_json::to_vec(message)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()
}
