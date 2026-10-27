use std::{borrow::Borrow, fmt};

use serde::{Deserializer, Serializer, de::Visitor};

pub fn serialize<S, V>(value: V, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    V: Borrow<u64>,
{
    serializer.serialize_str(&value.borrow().to_string())
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    struct DecimalVisitor;

    impl Visitor<'_> for DecimalVisitor {
        type Value = u64;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a decimal string or unsigned integer")
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
            Ok(value)
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            value.parse().map_err(E::custom)
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.visit_str(&value)
        }
    }

    deserializer.deserialize_any(DecimalVisitor)
}

pub mod option {
    use std::{borrow::Borrow, fmt};

    use serde::{Deserializer, Serializer, de::Visitor};

    pub fn serialize<S, V>(value: V, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        V: Borrow<Option<u64>>,
    {
        match value.borrow() {
            Some(value) => serializer.serialize_some(&value.to_string()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct OptionalDecimalVisitor;

        impl<'de> Visitor<'de> for OptionalDecimalVisitor {
            type Value = Option<u64>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("null, a decimal string, or an unsigned integer")
            }

            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(None)
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                super::deserialize(deserializer).map(Some)
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(None)
            }
        }

        deserializer.deserialize_option(OptionalDecimalVisitor)
    }
}
