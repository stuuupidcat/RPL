macro_rules! auto_derive {
    (
        #[auto_derive( $($derive:ident),* $(,)? )]
        $( #[$meta:meta] )*
        $vis:vis enum $ident:ident $variants:tt

    ) => {
        $( #[$meta] )*
        $vis enum $ident $variants

        $( $derive!( enum $ident $variants ); )*
    };

    (
        #[auto_derive( $($derive:ident),* $(,)? )]
        $( #[$meta:meta] )*
        $vis:vis struct $ident:ident $fields:tt
    ) => {
        $( #[$meta] )*
        $vis struct $ident $fields

        $( $derive!( struct $ident $fields ); )*
    };
}

macro_rules! From {
    (
        enum $ident:ident {
            $( $( #[$variant_meta:meta] )* $variant:ident $( ($ty:ty) )? ),* $(,)?
        }
    ) => {
        $( $(
        impl From<$ty> for $ident {
            fn from(inner: $ty) -> Self {
                $ident::$variant(inner)
            }
        }
        )? )*
    };
    (
        struct $ident:ident {
            $field_vis:vis $field:ident: $ty:ty,
        }
    ) => {
        impl From<$ty> for $ident {
            fn from($field: $ty) -> Self {
                $ident { $field }
            }
        }
    }
}
