use gel_protogen::prelude::*;

protocol!(
    /// Scalar Types
    struct Int16Value<'a> {
        value: i16,
    }

    struct Int32Value<'a> {
        value: i32,
    }

    struct Int64Value<'a> {
        value: i64,
    }

    struct Float32Value<'a> {
        value: f32,
    }

    struct Float64Value<'a> {
        value: f64,
    }

    struct BoolValue<'a> {
        value: u8, // 0 or 1
    }

    struct StringValue<'a> {
        value: RestString<'a>,
    }

    struct BytesValue<'a> {
        value: Rest<'a>,
    }

    struct UuidValue<'a> {
        value: [u8; 16],
    }

    struct JsonValue<'a> {
        format: u8, // Always 1
        value: RestString<'a>,
    }

    // Temporal Types
    struct DatetimeValue<'a> {
        micros: i64,
    }

    struct LocalDatetimeValue<'a> {
        micros: i64,
    }

    struct LocalDateValue<'a> {
        days: i32,
    }

    struct LocalTimeValue<'a> {
        micros: i64,
    }

    struct DurationValue<'a> {
        micros: i64,
        reserved1: u32 = 0,
        reserved2: u32 = 0,
    }

    struct RelativeDurationValue<'a> {
        micros: i64,
        days: i32,
        months: i32,
    }

    struct DateDurationValue<'a> {
        reserved: i64 = 0,
        days: i32,
        months: i32,
    }

    struct DecimalValue<'a> {
        ndigits: u16,
        weight: i16,
        sign: u16, // 0x0000 or 0x4000
        decimal_digits: u16,
        digits: RestArray<'a, u16>,
    }

    struct BigIntValue<'a> {
        ndigits: u16,
        weight: i16,
        sign: u16,  // 0x0000 or 0x4000
        reserved: u16 = 0,
        digits: RestArray<'a, u16>,
    }

    struct ArrayValue<'a> {
        ndims: u32,
        reserved0: u32 = 0,
        reserved1: u32 = 0,
        length: u32,
        lower: u32 = 1,
        elements: RestArray<'a, Encoded<'a>>,
    }

    struct TupleValue<'a> {
        nelements: u32,
        elements: RestArray<'a, Element<'a>>,
    }

    struct NamedTupleValue<'a> {
        nelements: u32,
        fields: RestArray<'a, Encoded<'a>>,
    }

    struct ObjectValue<'a> {
        nelements: u32,
        fields: RestArray<'a, ObjectElement<'a>>,
    }

    struct SetValue<'a> {
        ndims: u32,
        reserved0: u32 = 0,
        reserved1: u32 = 0,
        length: u32,
        lower: u32 = 1,
        elements: RestArray<'a, Encoded<'a>>,
    }

    /// Sets of arrays are a special case. Each array is wrapped in an Envelope.
    struct ArrayEnvelope<'a> {
        length: u32,
        nelems: u32,
        reserved: u32 = 0,
        elements: RestArray<'a, ArrayValue<'a>>,
    }

    /// Elements for tuples, sets, and arrays.
    struct Element<'a> {
        reserved: u32 = 0,
        data: Array<'a, u32, u8>,
    }

    /// Elements for objects, nullable.
    struct ObjectElement<'a> {
        index: u32,
        data: Encoded<'a>,
    }

    struct RangeValue<'a> {
        flags: u8, // Combination of EMPTY, LB_INC, UB_INC, LB_INF, UB_INF
        bounds: RestArray<'a, Encoded<'a>>,
    }

    struct MultiRangeValue<'a> {
        ranges: RestArray<'a, LengthPrefixed<RangeValue<'a>>>,
    }

    struct EnumValue<'a> {
        value: RestString<'a>,
    }

    struct PostGisGeometryValue<'a> {
        value: Rest<'a>,
    }

    struct PostGisGeographyValue<'a> {
        value: Rest<'a>,
    }

    struct PostGisBox2dValue<'a> {
        value: Rest<'a>,
    }

    struct PostGisBox3dValue<'a> {
        value: Rest<'a>,
    }

    struct VectorValue<'a> {
        length: u16,
        reserved: u16 = 0,
        values: RestArray<'a, f32>,
    }
);
