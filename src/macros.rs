/// Creates an `enum` with the given `name` and `variants`.
/// 
/// Also adds the [TryFrom<T>]
/// 
/// # Usage
/// 
/// ```rust
/// num_enum!{ name <T> {
///     variant1 = 0,
///     variant2 = 0xF0,
///     // ...
/// }}
/// ```
macro_rules! num_enum {
    ($name:ident <$T:ty> { $($variant:ident = $value:literal),* $(,)?}) => {
        #[derive(Debug, Clone, Serialize, std::cmp::Eq, std::cmp::PartialEq)]
        pub enum $name {
            $($variant = $value),*
        }

        impl TryFrom<$T> for $name {
            type Error = &'static str;
        
            fn try_from(value: $T) -> Result<$name, Self::Error> {
                match value {
                    $(
                        x if x == $name::$variant as $T => {return Ok($name::$variant);}
                    ),*
                    _ => Err("Not found")
                }
            }
        }
    };
    ($name:ident <$T:ty> { $($variant:ident),* $(,)?}) => {
        #[derive(Debug, Clone, Serialize, std::cmp::Eq, std::cmp::PartialEq)]
        pub enum $name {
            $($variant),*
        }

        impl TryFrom<$T> for $name {
            type Error = &'static str;
        
            fn try_from(value: $T) -> Result<$name, Self::Error> {
                match value {
                    $(
                        x if x == $name::$variant as $T => {return Ok($name::$variant);}
                    ),*
                    _ => Err("Not found")
                }
            }
        }
    };
}
