use serde_json::{json, Value};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const MAX_TURNS: usize = 4;

fn run(_prompt: &str, _ask: impl FnMut(&Value) -> Result<Value>) -> Result<String> {
    Err("agent loop not implemented".into())
}

fn main() {}

#[cfg(test)]
mod tests;
