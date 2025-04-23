/// Performs a first-pass parse on a struct, filling out some additional
/// metadata that makes the jobs of further macro passes much simpler.
///
/// This macro takes a `next` parameter which allows you to funnel the
/// structured data from the macro into the next macro.
///
/// This is a "push-down automation" and refers to how metadata and parsed
/// information are "pushed down" through the macro’s recursive structure. Each
/// level of the macro adds its own layer of processing and metadata
/// accumulation, eventually leading to the final output.
///
/// It begins by extracting and analyzing the fields of the `struct`, capturing
/// associated metadata such as attributes and types. This macro takes a `next`
/// parameter, which is another macro to be invoked after the current one
/// completes its task, allowing for a seamless chaining of macros where each
/// one builds upon the results of the previous.
///
/// The macro first makes some initial classifications of the fields based on
/// their types, then processes presence or absence of values, and finally
/// handles missing documentation.
///
/// As it processes each field, the macro recursively calls itself, accumulating
/// metadata and updating the state.
///
/// Once all fields have been processed, the macro enters the final stage, where
/// it reconstructs an enriched `struct`-like data blob using the accumulated
/// metadata. It then passes this enriched `struct` to the `next` macro for
/// further processing.
#[doc(hidden)]
#[macro_export]
macro_rules! struct_elaborate {
    (
        $next:ident $( ($($next_args:tt)*) )? =>
        $( #[ $sdoc:meta ] )*
        struct $name:ident <$lt:lifetime> $(: $super:ident)? {
            $(
                $( #[ doc = $fdoc:literal ] )* $field:ident :
                    $ty:ty
                    $( = $value:literal)?
            ),*
            $(,)?
        }
    ) => {
        // paste! is necessary here because it allows us to re-interpret a "ty"
        // as an explicit type pattern below.
        $crate::paste!($crate::struct_elaborate!(__builder_type__
            fields($(
                [
                    // Note that we double the type so we can re-use some output
                    // patterns in `__builder_type__`
                    type( $ty )( $ty ),
                    value($($value)?),
                    docs($($fdoc)*),
                    name($field),
                ]
            )*)
            // Accumulator for field data.
            accum()
            // Save the original struct parts so we can build the remainder of
            // the struct at the end.
            original($next $( ($($next_args)*) )? 
                => $(#[$sdoc])* struct $name <$lt> $(: $super)? {})
        ););
    };

    // End of push-down automation - jumps to `__finalize__`
    (__builder_type__ fields() accum($($faccum:tt)*) original($($original:tt)*)) => {
        $crate::struct_elaborate!(__finalize__ accum($($faccum)*) original($($original)*));
    };

    // Translate 'len' to Length (with auto value).
    (__builder_type__ fields([type(len)(len), value(), $($rest:tt)*] $($frest:tt)*) $($srest:tt)*) => {
        $crate::struct_elaborate!(__builder_docs__ fields([type($crate::prelude::Length), value(auto=auto), $($rest)*] $($frest)*) $($srest)*);
    };
    // Translate 'len' to Length (with a value present).
    (__builder_type__ fields([type(len)(len), value($($value:tt)+), $($rest:tt)*] $($frest:tt)*) $($srest:tt)*) => {
        $crate::struct_elaborate!(__builder_docs__ fields([type($crate::prelude::Length), value(value=($($value)*)), $($rest)*] $($frest)*) $($srest)*);
    };
    // Translate fixed-size arrays to FixedArray.
    (__builder_type__ fields([type([$elem:ty; $len:literal])($ty:ty), $($rest:tt)*] $($frest:tt)*) $($srest:tt)*) => {
        $crate::struct_elaborate!(__builder_value__ fields([type([$elem;$len]), $($rest)*] $($frest)*) $($srest)*);
    };

    // Fallback for other types - variable sized
    (__builder_type__ fields([type($ty:ty)($ty2:ty), $($rest:tt)*] $($frest:tt)*) $($srest:tt)*) => {
        $crate::struct_elaborate!(__builder_value__ fields([type($ty), $($rest)*] $($frest)*) $($srest)*);
    };

    // Next, mark the presence or absence of a value
    (__builder_value__ fields([
        type($ty:ty), value(), $($rest:tt)*
    ] $($frest:tt)*) $($srest:tt)*) => {
        $crate::struct_elaborate!(__builder_docs__ fields([type($ty), value(no_value=no_value), $($rest)*] $($frest)*) $($srest)*);
    };
    (__builder_value__ fields([
        type($ty:ty), value($($value:tt)+), $($rest:tt)*
    ] $($frest:tt)*) $($srest:tt)*) => {
        $crate::struct_elaborate!(__builder_docs__ fields([type($ty), value(value=($($value)*)), $($rest)*] $($frest)*) $($srest)*);
    };

    // Next, handle missing docs by generating a stand-in.
    (__builder_docs__ fields([
        type($ty:ty), value($($value:tt)*), docs(), name($field:ident), $($rest:tt)*
    ] $($frest:tt)*) $($srest:tt)*) => {
        $crate::struct_elaborate!(__builder__ fields([type($ty), value($($value)*), docs(concat!("`", stringify!($field), "` field.")), name($field), $($rest)*] $($frest)*) $($srest)*);
    };
    (__builder_docs__ fields([
        type($ty:ty), value($($value:tt)*), docs($($fdoc:literal)+), $($rest:tt)*
    ] $($frest:tt)*) $($srest:tt)*) => {
        $crate::struct_elaborate!(__builder__ fields([type($ty), value($($value)*), docs(concat!($($fdoc)+)), $($rest)*] $($frest)*) $($srest)*);
    };


    // Push down the field to the accumulator
    (__builder__ fields([
        type($ty:ty), value($($value:tt)*), docs($fdoc:expr), name($field:ident), $($rest:tt)*
    ] $($frest:tt)*) accum($($faccum:tt)*) original($($original:tt)*)) => {
        $crate::struct_elaborate!(__builder_type__ fields($($frest)*) accum(
            $($faccum)*
            {
                name($field),
                type($ty),
                value($($value)*),
                docs($fdoc),
            },
        ) original($($original)*));
    };

    // Write the final "elaborated" struct into a call to the `next` macro.
    (__finalize__ 
        accum($($accum:tt)*) original($next:ident $( ($($next_args:tt)*) )? => 
        $( #[ $sdoc:meta ] )* struct $name:ident <$lt:lifetime> $(: $super:ident)? {})
    ) => {
        $next ! (
            $( $($next_args)* , )?
            struct $name <$lt> {
                super($($super)?),
                docs($($sdoc),*),
                fields(
                    $($accum)*
                ),
            }
        );
    }
}

/// Generates a protocol definition from a Rust-like DSL.
///
/// LIMITATION: Enums must appear after structs.
///
/// ```nocompile
/// struct Foo {
///     bar: u8,
///     baz: u16 = 123,
/// }
///
/// #[repr(u8)]
/// enum MyEnum {
///     A = 1,
///     B = 2,
/// }
/// ```
#[doc(hidden)]
#[macro_export]
macro_rules! __protocol {
    (
        $( $( #[ doc = $sdoc:literal ] )*
            struct $name:ident <$lt:lifetime> $(: $super:ident)? { $($struct:tt)+ }
        )+
        $(  #[repr($repr:ty)] $( #[ doc = $edoc:literal ] )* 
            enum $ename:ident { $($(#[$default:meta])? $emname:ident = $emvalue:literal),+ $(,)? } 
        )*
    ) => {

        use $crate::protocol_builder;
        use $crate::prelude::*;

        $(
            $crate::paste!(
                $crate::struct_elaborate!(protocol_builder(__struct__) => $( #[ doc = $sdoc ] )* struct $name <$lt> $(: $super)? { $($struct)+ } );
                $crate::struct_elaborate!(protocol_builder(__meta__) => $( #[ doc = $sdoc ] )* struct $name <$lt> $(: $super)? { $($struct)+ } );
                $crate::struct_elaborate!(protocol_builder(__builder__) => $( #[ doc = $sdoc ] )* struct $name <$lt> $(: $super)? { $($struct)+ } );
            );
        )+

        $(
            $crate::paste!(
                $(#[doc = $edoc])*
                #[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
                #[repr($repr)]
                pub enum $ename {
                    $($(#[$default])? $emname = $emvalue),+
                }

                $crate::declare_type!($ename, flags=[enum], alias=$repr, {}

                );
            );
        )*
    };
}

#[doc(inline)]
pub use __protocol as protocol;

/// Simple conditional macro to check whether values are present.
#[macro_export]
#[doc(hidden)]
macro_rules! r#if {
    (__is_empty__ [] {$($true:tt)*} else {$($false:tt)*}) => {
        $($true)*
    };
    (__is_empty__ [$($x:tt)+] {$($true:tt)*} else {$($false:tt)*}) => {
        $($false)*
    };
    (__has__ [$($x:tt)+] {$($true:tt)*}) => {
        $($true)*
    };
    (__has__ [] {$($true:tt)*}) => {
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! protocol_builder {
    (__struct__, struct $name:ident <$lt:lifetime> {
        super($($super:ident)?),
        docs($($sdoc:meta),*),
        fields($({
            name($field:ident),
            type($type:ty),
            value($(value = ($value:expr))? $(no_value = $no_value:ident)? $(auto = $auto:ident)?),
            docs($fdoc:expr),
            $($rest:tt)*
        },)*),
    }) => {
        $crate::paste!(
            $( #[$sdoc] )?
            #[doc = concat!("\n\nAvailable fields: \n\n" $(
                , " - [`", stringify!($field), "`](Self::", stringify!($field), "()): ", $fdoc,
                $( "  (value = `", stringify!($value), "`)", )?
                "\n\n"
            )* )]
            #[derive(Copy, Clone, Default)]
            pub struct $name<$lt> {
                pub(crate) buf: &$lt [u8],
                $(
                    pub $field: $type,
                )*
            }

            impl PartialEq for $name<'_> {
                fn eq(&self, other: &Self) -> bool {
                    self.buf.eq(other.buf)
                }
            }

            impl Eq for $name<'_> {}

            impl std::fmt::Debug for $name<'_> {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    let mut s = f.debug_struct(stringify!($name));
                    $(
                        s.field(stringify!($field), &self.$field);
                    )*
                    s.finish()
                }
            }

            impl AsRef<[u8]> for $name<'_> {
                fn as_ref(&self) -> &[u8] {
                    self.buf.as_ref()
                }
            }

            impl $crate::gen2::StructNew for $name<'_> {
                fn new<'a>(buf: &'a [u8]) -> Result<Self::Struct<'a>, ParseError> {
                    Self::Struct::<'a>::new(buf)
                }
            }

            #[allow(unused)]
            impl <'a> $name<'a> {
                /// Checks the constant values for this struct to determine whether
                /// this message matches.
                #[inline]
                pub const fn is_buffer(buf: &'a [u8]) -> bool {
                    <Self as $crate::gen2::StructMeta>::FIELDS.matches_field_constants(buf)
                }

                /// Creates a new instance of this struct from a given buffer.
                #[inline]
                pub fn new(mut buf: &'a [u8]) -> Result<Self, ParseError> {
                    let mut new = $name::<'a>::default();
                    $(
                        new.$field = <$type as DataType>::decode(&mut buf)?;
                    )*
                    Ok(new)
                }

                pub fn to_vec(self) -> Vec<u8> {
                    self.buf.to_vec()
                }
            }
        );
    };

    (__meta__, struct $name:ident <$lt:lifetime> {
        super($($super:ident)?),
        docs($($sdoc:meta),*),
        fields($({
            name($field:ident),
            type($type:ty),
            value($(value = ($value:expr))? $(no_value = $no_value:ident)? $(auto = $auto:ident)?),
            docs($fdoc:expr),
            $($rest:tt)*
        },)*),
    }) => {
        $crate::paste!(
            #[allow(unused)]
            #[allow(non_camel_case_types)]
            #[derive(Eq, PartialEq)]
            #[repr(u8)]
            enum [<$name Fields>] {
                $(
                    $field,
                )*
            }

            #[allow(unused)]
            impl $name<'_> {
                // pub const FIELD_COUNT: usize = [$(stringify!($field)),*].len();
                // $($(pub const [<$field:upper _VALUE>]: <$type as $crate::prelude::DataType>::WithLifetime<'static> = super::access::FieldAccess::<$type>::constant($value as usize);)?)*
            }

            /// Implements a trait containing the fields of the struct, allowing
            /// us to compute some useful things.
            impl <$lt> $crate::gen2::StructMeta for $name<$lt> {
                const FIELDS: $crate::gen2::StructFields = $crate::gen2::StructFields::new(&
                    $crate::gen2::StructFieldComputed::new([
                        $(
                            $crate::gen2::StructField {
                                name: stringify!($field),
                                meta: &<$type as $crate::prelude::DataType>::META,
                                value: $crate::r#if!(__is_empty__ [$($value)?] { None } else { Some($($value)? as usize) }),
                            },
                        )*
                    ]));

                type Struct<'__struct> = $name<'__struct>;
            }

            /// Implements a trait indicating that the struct has a fixed size.
            /// This needs to be a trait-generic rather than and associated
            /// constant for us to use elsewhere.
            impl $crate::gen2::StructAttributeFixedSize<{<$name<'_> as $crate::gen2::StructMeta>::IS_FIXED_SIZE}> for $name<'_> {
            }

            /// Implements a trait indicating that the struct has a length field.
            impl $crate::gen2::StructAttributeHasLengthField<{<$name<'_> as $crate::gen2::StructMeta>::HAS_LENGTH_FIELD}> for $name<'_> {
            }

            /// Implements a trait indicating that the struct has a field count.
            impl $crate::gen2::StructAttributeFieldCount<{<$name<'_> as $crate::gen2::StructMeta>::FIELD_COUNT}> for $name<'_> {
            }

            $crate::declare_type!($name<'a>, builder: [<$name Builder>] <'a>, flags=[struct],
            {
                fn decode(buf: &mut &[u8]) -> Result<$name<'_>, $crate::ParseError> {
                    let mut buf = $crate::gen2::StructDemarcatedBuffer::new(buf)?;
                    Ok(S::new(buf.as_ref())?)
                }

                const fn encode(mut buf: $crate::BufWriter, value: &S) -> $crate::BufWriter {
                }
            }
            );
        );
    };

    (__builder__, struct $name:ident <$lt:lifetime> {
        super($($super:ident)?),
        docs($($sdoc:meta),*),
        fields($({
            name($field:ident),
            type($type:ty),
            value($(value = ($value:expr))? $(no_value = $no_value:ident)? $(auto = $auto:ident)?),
            docs($fdoc:expr),
            $($rest:tt)*
        },)*),
    }) => {
        $crate::paste!(
            $crate::r#if!(__is_empty__ [$($($no_value)?)*] {
                $( #[$sdoc] )?
                // No unfixed-value fields
                #[derive(::derive_more::Debug, Default, Eq, PartialEq)]
                pub struct [<$name Builder>]<'a> {
                    #[debug(skip)]
                    __no_fields_use_default: std::marker::PhantomData<&'a ()>
                }
            } else {
                $( #[$sdoc] )?
                #[derive(Debug, Default, Eq, PartialEq)]
                pub struct [<$name Builder>]<$lt> {
                    // Because of how macros may expand in the context of struct
                    // fields, we need to do a * repeat, then a ? repeat and
                    // somehow use $no_value in the remainder of the pattern.
                    $($(
                        #[doc = $fdoc]
                        pub $field: $crate::r#if!(__has__ [$no_value] {
                            <$type as $crate::prelude::DataType>::BuilderForStruct<$lt>
                        }),
                    )?)*
                }
            });

            impl [<$name Builder>]<'_> {
                // #[allow(unused)]
                // pub fn copy_to_buf(&self, buf: &mut $crate::BufWriter) {
                //     $(
                //         $crate::r#if!(__is_empty__ [$($value)?] {
                //             $crate::r#if!(__is_empty__ [$($auto)?] {
                //                 // value is no_value (present in builder)
                //                <$type as $crate::FieldAccessArray>::copy_to_buf(buf, &self.$field);
                //             } else {
                //                 // value is auto (not present in builder)
                //                 let auto_offset = buf.size();
                //                 <$type as $crate::FieldAccessArray>::copy_to_buf(buf, &0);
                //             });
                //         } else {
                //             // value is set, not present in builder
                //             <$type as $crate::FieldAccessArray>::copy_to_buf(buf, &($($value)? as usize as _));
                //         });
                //     )*

                //     $(
                //         $crate::r#if!(__has__ [$($auto)?] {
                //             $crate::FieldAccess::<$crate::meta::Length>::copy_to_buf_rewind(buf, auto_offset, buf.size() - auto_offset);
                //         });
                //     )*

                // }

                /// Convert this builder into a vector of bytes. This is generally
                /// not the most efficient way to perform serialization.
                #[allow(unused)]
                pub fn to_vec(&self) -> Vec<u8> {
                    // let mut vec = Vec::with_capacity(256);
                    // let mut buf = $crate::BufWriter::new(&mut vec);
                    // self.copy_to_buf(&mut buf);
                    // match buf.finish() {
                    //     Ok(size) => {
                    //         vec.truncate(size);
                    //         vec
                    //     },
                    //     Err(size) => {
                    //         vec.resize(size, 0);
                    //         let mut buf = $crate::BufWriter::new(&mut vec);
                    //         self.copy_to_buf(&mut buf);
                    //         // Will not fail this second time
                    //         let size = buf.finish().unwrap();
                    //         vec.truncate(size);
                    //         vec
                    //     }
                    // }
                    unimplemented!()
                }

                // #[allow(unused)]
                // pub const fn measure(&self) -> usize {
                //     let mut size = 0;

                //     $(
                //         $crate::r#if!(__is_empty__ [$($value)?] {
                //             $crate::r#if!(__is_empty__ [$($auto)?] {
                //                 let field_size = super::access::FieldAccess::<$type>::measure(&self.$field);
                //             } else {
                //                 let field_size = super::access::FieldAccess::<$type>::measure(&0);
                //             });
                //         } else {
                //             let field_size = super::access::FieldAccess::<$type>::measure(&0);
                //         });

                //         size += field_size;
                //     )*
                //     size
                // }
            }
        );
    };
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use crate::gen2::StructAttributeHasLengthField;

    mod fixed_only {
        use super::*;

        crate::protocol!(
            struct FixedOnly <'a> {
                a: u8,
            }
        );

        static_assertions::assert_impl_any!(FixedOnly::<'static>: StructAttributeHasLengthField<false>);
        static_assertions::assert_not_impl_any!(FixedOnly::<'static>: StructAttributeHasLengthField<true>);
    }

    mod fixed_only_value {
        crate::protocol!(
            struct FixedOnlyValue <'a> {
                a: u8 = 1,
            }
        );
    }

    mod mixed {
        crate::protocol!(
            struct Mixed <'a> {
                a: u8 = 1,
                s: ZTString<'a>,
            }
        );
    }

    mod docs {
        crate::protocol!(
            /// Docs
            struct Docs <'a> {
                /// Docs
                a: u8 = 1,
                /// Docs
                s: ZTString<'a>,
            }
        );
    }

    mod length {
        use super::*;

        crate::protocol!(
            struct WithLength <'a> {
                a: u8,
                l: len,
            }
        );

        static_assertions::assert_impl_any!(WithLength::<'static>: StructAttributeHasLengthField<true>);
        static_assertions::assert_not_impl_any!(WithLength::<'static>: StructAttributeHasLengthField<false>);
    }

    mod array {
        crate::protocol!(
            struct StaticArray <'a> {
                a: u8,
                l: [u8; 4],
            }
        );
    }

    mod string {
        crate::protocol!(
            struct HasLString <'a> {
                s: LString<'a>,
            }
        );
    }

    mod has_enum {
        crate::protocol!(
            struct HasEnum <'a> {
                e: MyEnum,
            }

            #[repr(u8)]
            enum MyEnum {
                #[default]
                A = 1,
                B = 2,
            }
        );
    }

    macro_rules! assert_stringify {
        (($($struct:tt)*), ($($expected:tt)*)) => {
            $crate::struct_elaborate!(assert_stringify(__internal__ ($($expected)*)) => $($struct)*);
        };
        (__internal__ ($($expected:tt)*), $($struct:tt)*) => {
            // We don't want whitespace to impact this comparison
            if stringify!($($struct)*).replace(char::is_whitespace, "") != stringify!($($expected)*).replace(char::is_whitespace, "") {
                assert_eq!(stringify!($($struct)*), stringify!($($expected)*));
            }
        };
    }

    // #[test]
    // fn empty_struct() {
    //     assert_stringify!((struct <'a> Foo {}), (struct <'a> Foo { super (), docs(), fields(), }));
    // }

    // #[test]
    // fn fixed_size_fields() {
    //     assert_stringify!((struct <'a> Foo {
    //                 a: u8,
    //                 b: u8,
    //             }), (struct Foo
    //     {
    //         super (),
    //         docs(),
    //         fields({
    //             name(a), type (u8), value(no_value = no_value),
    //             docs(concat!("`", stringify! (a), "` field.")),
    //         },
    //         {
    //             name(b), type (u8), value(no_value = no_value),
    //             docs(concat!("`", stringify! (b), "` field.")),
    //         },),
    //     }));
    // }

    // #[test]
    // fn mixed_fields() {
    //     assert_stringify!((struct Foo {
    //                 a: u8,
    //                 l: len,
    //                 s: ZTString,
    //                 c: i16,
    //                 d: [u8; 4],
    //                 e: ZTArray<ZTString>,
    //             }), (struct Foo
    //     {
    //         super (),
    //         docs(),
    //         fields({
    //             name(a), type (u8), value(no_value = no_value),
    //             docs(concat!("`", stringify! (a), "` field.")),
    //         },
    //         {
    //             name(l), type ($crate::meta::Length),
    //             value(auto = auto), docs(concat!("`", stringify! (l), "` field.")),
    //         },
    //         {
    //             name(s), type (ZTString),
    //             value(no_value = no_value),
    //             docs(concat!("`", stringify! (s), "` field.")),
    //         },
    //         {
    //             name(c), type (i16), value(no_value = no_value),
    //             docs(concat!("`", stringify! (c), "` field.")),
    //         },
    //         {
    //             name(d), type ($crate::meta::FixedArray<4, u8>),
    //             value(no_value = no_value),
    //             docs(concat!("`", stringify! (d), "` field.")),
    //         },
    //         {
    //             name(e), type (ZTArray<ZTString>),
    //             value(no_value = no_value),
    //             docs(concat!("`", stringify! (e), "` field.")),
    //         },
    //     ),
    //     }));
    // }
}
