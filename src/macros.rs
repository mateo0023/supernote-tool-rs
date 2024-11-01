use serde::Deserialize;

/// Creates an `enum` with the given `name` and `variants`.
/// 
/// Also adds the [TryFrom<T>], defaulting to `T = u8`.
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
        #[derive(Debug, Clone, Copy, serde::Serialize, std::cmp::Eq, std::cmp::PartialEq)]
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
        #[derive(Debug, Clone, Copy, serde::Serialize, std::cmp::Eq, std::cmp::PartialEq)]
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
    ($name:ident { $($variant:ident = $value:literal),* $(,)?}) => {
        num_enum! {$name <u8> { $($variant = $value),* } }
    };
    ($name:ident { $($variant:ident),* $(,)?}) => {
        #[derive(Debug, Clone, Copy, serde::Serialize, std::cmp::Eq, std::cmp::PartialEq)]
        pub enum $name {
            $($variant),*
        }

        impl TryFrom<u8> for $name {
            type Error = &'static str;
        
            fn try_from(value: u8) -> Result<$name, Self::Error> {
                match value {
                    $(
                        x if x == $name::$variant as u8 => {return Ok($name::$variant);}
                    ),*
                    _ => Err("Not found")
                }
            }
        }
    };
}

/// Given a previous `object: Option<T>` and the orginal text:
/// 1. Tries to convert the object to type U.
/// 2. Tries to deserialize the original text to the type U.
pub trait Upgradable<T> {
    fn upgrade_or_deserialize<'a, U>(self, text: &'a str) -> Option<U>
    where
        U: serde::Deserialize<'a> + From<T>;
}

impl<T> Upgradable<T> for Option<T> {
    fn upgrade_or_deserialize<'a, U>(self: Option<T>, text: &'a str) -> Option<U>
    where
        U: Deserialize<'a> + From<T>,
    {
        match self {
            Some(ok) => Some(ok.into()),
            None => serde_json::from_str(text).ok(),
        }
    }
}



/// 
/// `$text` needs to be a `&str`
/// 
/// You need to bring the [`Upgradable`] trait into
/// scope for the macro to compile.
macro_rules! back_compat_deserialize {
    ($text:expr, $($version:ty),+) => {{
        use $crate::macros::Upgradable;
        None$(.upgrade_or_deserialize::<$version>($text))+
    }};
}