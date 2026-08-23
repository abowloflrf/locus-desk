use serde::{Deserialize, Deserializer, de::DeserializeOwned};

use crate::error::{AppError, AppResult};

#[derive(Debug, Default)]
pub enum PatchField<T> {
    #[default]
    Missing,
    Null,
    Value(T),
}

impl<T> PatchField<T> {
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }

    pub fn into_required(self, field: &'static str) -> AppResult<Option<T>> {
        match self {
            Self::Missing => Ok(None),
            Self::Null => Err(AppError::bad_request(format!("{field} must not be null"))),
            Self::Value(value) => Ok(Some(value)),
        }
    }

    pub fn into_nullable(self) -> Option<Option<T>> {
        match self {
            Self::Missing => None,
            Self::Null => Some(None),
            Self::Value(value) => Some(Some(value)),
        }
    }
}

pub fn deserialize_patch_field<'de, D, T>(deserializer: D) -> Result<PatchField<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    Option::<T>::deserialize(deserializer).map(|value| match value {
        Some(value) => PatchField::Value(value),
        None => PatchField::Null,
    })
}
