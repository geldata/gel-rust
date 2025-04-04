/*!
Contains the [QueryResult](crate::query_result::QueryResult) trait.
*/

use std::sync::Arc;

use bytes::Bytes;

use gel_errors::{DescriptorMismatch, ProtocolEncodingError};
use gel_errors::{Error, ErrorKind};

use crate::codec::Codec;
use crate::descriptors::TypePos;
use crate::queryable::{Decoder, DescriptorContext, Queryable};
use crate::value::Value;

pub trait Sealed: Sized {}

/// A trait representing single result from a query.
///
/// This is implemented for scalars and tuples. To receive a shape from Gel
/// derive [`Queryable`](Queryable) for a structure. This will automatically
/// implement `QueryResult` for you.
pub trait QueryResult: Sealed {
    type State;
    fn prepare(ctx: &DescriptorContext, root_pos: TypePos) -> Result<Self::State, Error>;
    fn decode(state: &mut Self::State, msg: &Bytes) -> Result<Self, Error>;
}

impl<T: Queryable> Sealed for T {}

impl Sealed for Value {}

impl<T: Queryable> QueryResult for T {
    type State = (Decoder, T::Args);
    fn prepare(ctx: &DescriptorContext, root_pos: TypePos) -> Result<Self::State, Error> {
        let args = T::check_descriptor(ctx, root_pos).map_err(DescriptorMismatch::with_source)?;
        let decoder = Decoder {
            has_implicit_id: ctx.has_implicit_id,
            has_implicit_tid: ctx.has_implicit_tid,
            has_implicit_tname: ctx.has_implicit_tname,
        };
        Ok((decoder, args))
    }
    fn decode((decoder, args): &mut Self::State, msg: &Bytes) -> Result<Self, Error> {
        Queryable::decode(decoder, args, msg).map_err(ProtocolEncodingError::with_source)
    }
}

impl QueryResult for Value {
    type State = Arc<dyn Codec>;
    fn prepare(ctx: &DescriptorContext, root_pos: TypePos) -> Result<Arc<dyn Codec>, Error> {
        ctx.build_codec(root_pos)
    }
    fn decode(codec: &mut Arc<dyn Codec>, msg: &Bytes) -> Result<Self, Error> {
        let res = codec.decode(msg);

        match res {
            Ok(v) => Ok(v),
            Err(e) => {
                if let Some(bt) = snafu::ErrorCompat::backtrace(&e) {
                    eprintln!("{bt}");
                }
                Err(ProtocolEncodingError::with_source(e))
            }
        }
    }
}
