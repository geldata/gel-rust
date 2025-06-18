#[doc(hidden)]
#[macro_export]
macro_rules! __message_group {
    ($(#[$doc:meta])* $group:ident : $super:ident = [ $($message:ident),* $(,)? ] ) => {
        $crate::paste!(

        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[allow(unused)]
        pub enum $group {
            $(
                #[doc = concat!("Matched [`", stringify!($message), "`]")]
                $message
            ),*
        }

        pub enum [<$group Builder>]<'a> {
            $(
                $message(&'a dyn $crate::prelude::EncoderFor<$message<'static>>),
            )*
        }

        impl<'a> [<$group Builder>]<'a> {
            #[allow(private_bounds)]
            pub fn new<T>(message: impl [<Into $group Builder>]<'a, T>) -> Self {
                message.into_builder()
            }

            pub fn encode<'b>(&self, buf: &mut BufWriter<'b>) {
                match self {
                    $(
                        Self::$message(message) => message.encode_for(buf),
                    )*
                }
            }

            pub fn to_vec(self) -> Vec<u8> {
                match self {
                    $(
                        Self::$message(message) => EncoderForExt::to_vec(message),
                    )*
                }
            }
        }

        pub trait [<Into $group Builder>]<'a, T> {
            fn into_builder(self) -> [<$group Builder>]<'a>;
        }

        impl <'a, T, U> [<Into $group Builder>]<'a, T> for U where U: [<sealed_ $group:lower>]::[<$group BuilderTrait>]<'a, T> {
            fn into_builder(self) -> [<$group Builder>]<'a> {
                self.into_builder_private()
            }
        }

        mod [< sealed_ $group:lower>] {
            use super::*;
            pub(crate) trait [<$group BuilderTrait>]<'a, T>: Sized {
                fn into_builder_private(self) -> [<$group Builder>]<'a>;
            }
        }

        $(
        impl <'a, T> [< sealed_ $group:lower>]::[<$group BuilderTrait>]<'a, $message<'static>> for &'a T where T: $crate::prelude::EncoderFor<$message<'static>> {
            fn into_builder_private(self) -> [<$group Builder>]<'a> {
                [<$group Builder>]::$message(self)
            }
        }
        )*

        impl<'a> ::std::fmt::Debug for [<$group Builder>]<'a> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(
                        Self::$message(_) => write!(f, stringify!($message)),
                    )*
                }
            }
        }

        #[allow(unused)]
        pub trait [<$group Match>] {
            $(
                fn [<$message:snake>]<'a>(&mut self) -> Option<impl FnMut($message<'a>)> {
                    // No implementation by default
                    let mut opt = Some(|_| {});
                    opt.take();
                    opt
                }
            )*
            // fn unknown(&mut self, message: self::struct_defs::Message::Message) {
            //     // No implementation by default
            // }
        }

        #[allow(unused)]
        impl $group {
            pub fn identify(buf: &[u8]) -> Option<Self> {
                $(
                    if $message::is_buffer(buf) {
                        return Some(Self::$message);
                    }
                )*
                None
            }

        }
        );
    };
}

#[doc(inline)]
pub use __message_group as message_group;

/// Perform a match on a message.
///
/// ```rust
/// use gel_db_protocol::*;
/// use gel_db_protocol::test_protocol::*;
///
/// let buf = [b'?', 0, 0, 0, 4];
/// match_message!(Message::new(&buf), Backend {
///     (DataRow as data) => {
///         todo!();
///     },
///     unknown => {
///         eprintln!("Unknown message: {unknown:?}");
///     }
/// });
/// ```
#[doc(hidden)]
#[macro_export]
macro_rules! __match_message {
    ($buf:expr, $messages:ty {
        $(( $i1:path $(as $i2:ident )?) $(if $cond:expr)? => $impl:block,)*
        $unknown:ident => $unknown_impl:block $(,)?
    }) => {
        'block: {
            let __message: Result<_, $crate::prelude::ParseError> = $buf;
            let res = match __message {
                Ok(__message) => {
                    $(
                        if $($cond &&)? <$i1>::is_buffer(&__message.as_ref()) {
                            match(<$i1>::new(&__message.as_ref())) {
                                Ok(__tmp) => {
                                    $(let $i2 = __tmp;)?
                                    #[allow(unreachable_code)]
                                    break 'block ({ $impl })
                                }
                                Err(e) => Err(e)
                            }
                        } else
                    )*
                    {
                        Ok(__message)
                    }
                },
                Err(e) => Err(e)
            };
            {
                let $unknown = res;
                #[allow(unreachable_code)]
                break 'block ({ $unknown_impl })
            }
        }
    };
}

#[doc(inline)]
pub use __match_message as match_message;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;
    use crate::test_protocol;
    use crate::test_protocol::*;

    #[test]
    fn test_match() {
        let message = SyncBuilder::default().to_vec();
        let message = Message::new(&message);
        match_message!(message, Message {
            (DataRow as data_row) => {
                eprintln!("{data_row:?}");
                return;
            },
            unknown => {
                eprintln!("{unknown:?}");
                return;
            }
        });
    }

    #[test]
    fn dyn_message() {
        use test_protocol::IntoBackendBuilder;
        let msg = test_protocol::FixedLengthBuilder::default();
        let message = (&msg).into_builder();
        eprintln!("{message:?}");
    }
}
