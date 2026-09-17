// schema1
//
// An MCP tool's arguments are described to the model as JSON Schema. In rmcp
// you do not write that schema by hand: you derive it from a Rust struct, so
// the type you deserialize into and the contract the model sees are the same
// thing. Change the struct and the schema follows.
//
// `GreetArgs` below is missing the two derives that make that work, and its
// field has no doc comment. The doc comment matters: schemars copies it into
// the schema as the field's `description`, which is the only explanation of
// `name` the model will ever get.
//
// Nothing to run here. The tests are the exercise:
//
//     cargo run -- run schema1

use schemars::JsonSchema;
use serde::Deserialize;

// TODO: derive `Deserialize` (so rmcp can parse the JSON the model sends)
// and `JsonSchema` (so rmcp can describe this struct to the model).
#[derive(Debug)]
struct GreetArgs {
    // TODO: add a doc comment (`///`) that tells the model what this field is for.
    name: String,
}

fn main() {
    // Nothing to run yet. This exercise lives in its tests.
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The struct must accept what a client will actually send.
    #[test]
    fn parses_the_json_a_client_sends() {
        let args: GreetArgs = serde_json::from_str(r#"{"name": "Ada"}"#).expect("parse");
        assert_eq!(args.name, "Ada");
    }

    /// The schema the model sees is derived from the struct: `name` is a
    /// required string, and its doc comment became the description.
    #[test]
    fn schema_describes_name_for_the_model() {
        let schema = serde_json::to_value(schemars::schema_for!(GreetArgs)).expect("schema");

        assert_eq!(
            schema["properties"]["name"]["type"], "string",
            "name should be a string: {schema:#}"
        );
        let required = schema["required"].as_array();
        assert!(
            required.is_some_and(|r| r.iter().any(|v| v == "name")),
            "name should be required: {schema:#}"
        );
        let description = schema["properties"]["name"]["description"]
            .as_str()
            .unwrap_or("");
        assert!(
            !description.trim().is_empty(),
            "give `name` a doc comment; the model reads it as the description: {schema:#}"
        );
    }
}
