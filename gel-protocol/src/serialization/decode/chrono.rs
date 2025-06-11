use crate::errors::DecodeError;
use crate::serialization::decode::raw_scalar::RawCodec;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};

impl RawCodec<'_> for DateTime<Utc> {
    fn decode(buf: &[u8]) -> Result<Self, DecodeError> {
        crate::model::Datetime::decode(buf).map(Into::into)
    }
}

impl RawCodec<'_> for NaiveDateTime {
    fn decode(buf: &[u8]) -> Result<Self, DecodeError> {
        crate::model::LocalDatetime::decode(buf).map(Into::into)
    }
}

impl RawCodec<'_> for NaiveDate {
    fn decode(buf: &[u8]) -> Result<Self, DecodeError> {
        crate::model::LocalDate::decode(buf).map(Into::into)
    }
}

impl RawCodec<'_> for NaiveTime {
    fn decode(buf: &[u8]) -> Result<Self, DecodeError> {
        crate::model::LocalTime::decode(buf).map(Into::into)
    }
}
