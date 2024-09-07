#[macro_export] macro_rules! conv_ident {
    ($ident:ident, $new:ident) => { $new };
}

#[macro_export] macro_rules! error_creator {
    (
        $error_name:ident,
        $error_kind_name:ident,
        $( $kind:ident$(($val:ident))? => $string:expr ),*
    ) => {
        use $crate::conv_ident;
        use std::rc::Rc;
        #[derive(Clone, Eq, PartialEq, Hash)]
        pub enum $error_kind_name {
            $($kind $(($val))? ),*
        }
        
        #[derive(Clone, Eq, PartialEq, Hash)]
        pub struct $error_name {
            kind: $error_kind_name,
            message: Rc<String>,
        }
        
        impl std::fmt::Debug for $error_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { std::fmt::Display::fmt(self, f) }
        }
        
        impl std::fmt::Display for $error_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self.kind() {
                    $( $error_kind_name::$kind $((conv_ident!($val, val)))? => { 
                        $( if true { std::fmt::Display::fmt(&conv_ident!($val, val), f)?; } else )? 
                        { f.write_str($string)?; } 
                        if !self.message.is_empty() {
                            if !$string.trim().is_empty() $(|| conv_ident!($val, true))? {
                                f.write_str(", ")?;
                            }
                            f.write_str(self.message.as_str())
                        } else { Ok(()) } } ),*
                }
            }
        }
        
        impl std::error::Error for $error_name { }
        
        type Result<T> = std::result::Result<T, $error_name>;
        
        impl $error_name {
            #[allow(unused)]
            pub fn new(kind: $error_kind_name) -> Self {
                Self {
                    kind,
                    message: Rc::default(),
                }
            }
            
            #[allow(unused)]
            pub fn with_message(kind: $error_kind_name, message: impl Into<String>) -> Self {
                Self {
                    kind,
                    message: Rc::new(message.into()),
                }
            }
        
            #[allow(unused)]
            pub fn kind(&self) -> &$error_kind_name {
                &self.kind
            }
        }
        
        $( $(
        
        impl core::convert::From<$val> for $error_name {
            fn from(value: $val) -> Self {
                $error_name::new($error_kind_name::$val(value))
            }
        }
        
        )? )*
    };
}