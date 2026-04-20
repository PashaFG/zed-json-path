use zed_extension_api as zed;

struct JsonPathExtension;

impl zed::Extension for JsonPathExtension {
    fn new() -> Self {
        Self
    }
}

zed::register_extension!(JsonPathExtension);
