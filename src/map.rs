//! This module defines a macro to create a map type with a specified key and value type.

macro_rules! define_map_type {
    ($name:ident, $key_type:ty, $value_type:ty) => {
        #[derive(PartialEq, Debug, Clone, Default)]
        pub struct $name(HashMap<$key_type, $value_type>);

        impl $name {
            /// Create a new, empty map
            pub fn new() -> Self {
                Self::default()
            }

            /// Check if the map is empty
            pub fn is_empty(&self) -> bool {
                self.0.is_empty()
            }

            /// Insert a value into the map
            pub fn insert(&mut self, key: $key_type, value: $value_type) -> anyhow::Result<()> {
                if self.0.contains_key(&key) {
                    return Err(anyhow::anyhow!("Key {:?} already exists in the map", key));
                }
                self.0.insert(key, value);
                Ok(())
            }

            /// Retrieve a value from the map
            /// Assumes the key exists in the map, otherwise will panic
            pub fn get(&self, key: $key_type) -> $value_type {
                self.0
                    .get(&key)
                    .unwrap_or_else(|| panic!("Key {:?} not found in the map", key))
                    .clone()
            }

            /// Check if the map contains a specific key
            pub fn contains_key(&self, key: &$key_type) -> bool {
                self.0.contains_key(key)
            }
        }
    };
}
pub(crate) use define_map_type;
