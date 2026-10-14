use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionSchema {
    pub name: String,
    pub fields: Vec<FieldDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDef {
    pub name: String,
    pub field_type: FieldType,
    pub required: bool,
    pub indexed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    String,
    Integer,
    Float,
    Boolean,
    DateTime,
    Uuid,
    Json,
    Reference(String),
    Array(Box<FieldType>),
}
