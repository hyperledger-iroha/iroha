//! Macros for implementing common traits for ID types.
macro_rules! string_id {
    ($($ty:ty),+ $(,)?) => {
        $(
            #[cfg(feature = "json")]
            impl norito::json::FastJsonWrite for $ty {
                fn write_json(&self, out: &mut String) {
                    let repr = self.to_string();
                    norito::json::JsonSerialize::json_serialize(&repr, out);
                }
                fn write_json_to(
                    &self,
                    out: &mut dyn norito::json::JsonWriteSink,
                ) -> Result<(), norito::json::BoundedJsonError> {
                    norito::json::write_json_string_to(&self.to_string(), out)
                }
            }
            #[cfg(feature = "json")]
            impl norito::json::JsonDeserialize for $ty {
                fn json_deserialize(
                    parser: &mut norito::json::Parser<'_>,
                ) -> Result<Self, norito::json::Error> {
                    let value = parser.parse_string()?;
                    value
                        .parse()
                        .map_err(|err| norito::json::Error::Message(format!("{err}")))
                }
            }
        )+
    };
}
