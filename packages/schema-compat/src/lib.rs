use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemaType {
    String,
    Number,
    Integer,
    Boolean,
    Object,
    Array,
    Null,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdditionalProperties {
    Allowed,
    Disallowed,
    Schema(Box<JsonSchema>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct JsonSchema {
    pub schema_type: Option<SchemaType>,
    pub description: Option<String>,
    pub properties: BTreeMap<String, JsonSchema>,
    pub required: Vec<String>,
    pub items: Option<Box<JsonSchema>>,
    pub enum_values: Vec<Value>,
    pub nullable: bool,
    pub additional_properties: Option<AdditionalProperties>,
    pub any_of: Vec<JsonSchema>,
}

impl JsonSchema {
    pub fn string() -> Self {
        Self {
            schema_type: Some(SchemaType::String),
            ..Self::default()
        }
    }

    pub fn object() -> Self {
        Self {
            schema_type: Some(SchemaType::Object),
            ..Self::default()
        }
    }

    pub fn array(items: JsonSchema) -> Self {
        Self {
            schema_type: Some(SchemaType::Array),
            items: Some(Box::new(items)),
            ..Self::default()
        }
    }

    pub fn property(mut self, name: impl Into<String>, schema: JsonSchema, required: bool) -> Self {
        let name = name.into();
        self.properties.insert(name.clone(), schema);
        if required && !self.required.contains(&name) {
            self.required.push(name);
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelInformation {
    pub model_id: String,
    pub provider: String,
    pub supports_structured_outputs: bool,
}

pub trait SchemaCompatLayer {
    fn model(&self) -> &ModelInformation;
    fn apply(&self, schema: JsonSchema) -> JsonSchema;
}

#[derive(Debug, Clone)]
pub struct OpenAISchemaCompatLayer {
    model: ModelInformation,
}

impl OpenAISchemaCompatLayer {
    pub fn new(model: ModelInformation) -> Self {
        Self { model }
    }
}

impl SchemaCompatLayer for OpenAISchemaCompatLayer {
    fn model(&self) -> &ModelInformation {
        &self.model
    }

    fn apply(&self, schema: JsonSchema) -> JsonSchema {
        if !self.model.supports_structured_outputs {
            return schema;
        }

        transform_for_openai(schema)
    }
}

#[derive(Debug, Clone)]
pub struct AnthropicSchemaCompatLayer {
    model: ModelInformation,
}

impl AnthropicSchemaCompatLayer {
    pub fn new(model: ModelInformation) -> Self {
        Self { model }
    }
}

impl SchemaCompatLayer for AnthropicSchemaCompatLayer {
    fn model(&self) -> &ModelInformation {
        &self.model
    }

    fn apply(&self, schema: JsonSchema) -> JsonSchema {
        if !self.model.supports_structured_outputs {
            return schema;
        }

        transform_for_anthropic(schema)
    }
}

pub fn apply_provider_compat(schema: JsonSchema, model: &ModelInformation) -> JsonSchema {
    match model.provider.as_str() {
        "openai" => OpenAISchemaCompatLayer::new(model.clone()).apply(schema),
        "anthropic" => AnthropicSchemaCompatLayer::new(model.clone()).apply(schema),
        _ => schema,
    }
}

fn transform_for_openai(mut schema: JsonSchema) -> JsonSchema {
    schema.items = schema
        .items
        .map(|items| Box::new(transform_for_openai(*items)));
    schema.any_of = schema
        .any_of
        .into_iter()
        .map(transform_for_openai)
        .collect();

    if let Some(AdditionalProperties::Schema(inner)) = schema.additional_properties.take() {
        schema.additional_properties = Some(AdditionalProperties::Schema(Box::new(
            transform_for_openai(*inner),
        )));
    }

    if schema.schema_type == Some(SchemaType::Object) {
        let existing_required = schema.required.clone();
        let property_names: Vec<String> = schema.properties.keys().cloned().collect();

        schema.properties = schema
            .properties
            .into_iter()
            .map(|(name, mut property)| {
                property = transform_for_openai(property);
                if !existing_required.contains(&name) {
                    property.nullable = true;
                }
                (name, property)
            })
            .collect();

        schema.required = property_names;
        if schema.additional_properties.is_none() {
            schema.additional_properties = Some(AdditionalProperties::Disallowed);
        }
    }

    schema
}

fn transform_for_anthropic(mut schema: JsonSchema) -> JsonSchema {
    schema.items = schema
        .items
        .map(|items| Box::new(transform_for_anthropic(*items)));
    schema.any_of = schema
        .any_of
        .into_iter()
        .map(transform_for_anthropic)
        .collect();

    if let Some(AdditionalProperties::Schema(inner)) = schema.additional_properties.take() {
        schema.additional_properties = Some(AdditionalProperties::Schema(Box::new(
            transform_for_anthropic(*inner),
        )));
    }

    if schema.schema_type == Some(SchemaType::Object) && schema.additional_properties.is_none() {
        schema.additional_properties = Some(AdditionalProperties::Disallowed);
    }

    schema.properties = schema
        .properties
        .into_iter()
        .map(|(name, property)| (name, transform_for_anthropic(property)))
        .collect();

    schema
}

#[cfg(test)]
mod tests {
    use super::{
        AdditionalProperties, AnthropicSchemaCompatLayer, JsonSchema, ModelInformation,
        OpenAISchemaCompatLayer, SchemaCompatLayer, SchemaType, apply_provider_compat,
    };

    #[test]
    fn openai_compat_makes_optional_fields_required_and_nullable() {
        let schema = JsonSchema::object()
            .property("name", JsonSchema::string(), true)
            .property("nickname", JsonSchema::string(), false);

        let model = ModelInformation {
            model_id: "gpt-4o".into(),
            provider: "openai".into(),
            supports_structured_outputs: true,
        };

        let compat = OpenAISchemaCompatLayer::new(model);
        let transformed = compat.apply(schema);

        assert_eq!(transformed.required, vec!["name", "nickname"]);
        assert_eq!(
            transformed.additional_properties,
            Some(AdditionalProperties::Disallowed)
        );
        assert_eq!(transformed.properties["name"].nullable, false);
        assert_eq!(transformed.properties["nickname"].nullable, true);
    }

    #[test]
    fn anthropic_compat_preserves_optional_fields() {
        let schema = JsonSchema::object()
            .property("required", JsonSchema::string(), true)
            .property("optional", JsonSchema::string(), false);

        let model = ModelInformation {
            model_id: "claude-sonnet".into(),
            provider: "anthropic".into(),
            supports_structured_outputs: true,
        };

        let compat = AnthropicSchemaCompatLayer::new(model);
        let transformed = compat.apply(schema);

        assert_eq!(transformed.required, vec!["required"]);
        assert_eq!(transformed.properties["optional"].nullable, false);
        assert_eq!(
            transformed.additional_properties,
            Some(AdditionalProperties::Disallowed)
        );
    }

    #[test]
    fn apply_provider_compat_is_noop_without_structured_outputs() {
        let schema = JsonSchema {
            schema_type: Some(SchemaType::Object),
            ..JsonSchema::default()
        };
        let model = ModelInformation {
            model_id: "legacy".into(),
            provider: "openai".into(),
            supports_structured_outputs: false,
        };

        assert_eq!(apply_provider_compat(schema.clone(), &model), schema);
    }
}
