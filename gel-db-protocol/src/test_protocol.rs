//! A pseudo-Postgres protocol for testing.
use crate::gen::protocol;

protocol!(
    struct Message<'a> {
        /// The message type.
        mtype: u8,
        /// The length of the message contents in bytes, including self.
        mlen: len,
        /// The message contents.
        data: Rest<'a>,
    }

    /// The `CommandComplete` struct represents a message indicating the successful completion of a command.
    struct CommandComplete<'a>: Message {
        /// Identifies the message as a command-completed response.
        mtype: u8 = 'C',
        /// Length of message contents in bytes, including self.
        mlen: len,
        /// The command tag.
        tag: ZTString<'a>,
    }

    /// The `Sync` message is used to synchronize the client and server.
    struct Sync<'a>: Message {
        /// Identifies the message as a synchronization request.
        mtype: u8 = 'S',
        /// Length of message contents in bytes, including self.
        mlen: len,
    }

    /// The `DataRow` message represents a row of data returned from a query.
    struct DataRow<'a>: Message {
        /// Identifies the message as a data row.
        mtype: u8 = 'D',
        /// Length of message contents in bytes, including self.
        mlen: len,
        /// The values in the row.
        values: Array<'a, i16, Encoded<'a>>,
    }

    struct QueryType<'a> {
        /// The type of the query parameter.
        typ: QueryParameterType,
        /// The length of the query parameter.
        length: u32,
        /// The metadata of the query parameter.
        meta: Array<'a, u32, u8>,
    }

    struct Query<'a>: Message {
        /// Identifies the message as a query.
        mtype: u8 = 'Q',
        /// Length of message contents in bytes, including self.
        mlen: len,
        /// The query string.
        query: ZTString<'a>,
        /// The types of the query parameters.
        types: Array<'a, i16, QueryType<'a>>,
    }

    /// A fixed-length message.
    struct FixedLength<'a>: Message {
        /// Identifies the message as a fixed-length message.
        mtype: u8 = 'F',
        /// Length of message contents in bytes, including self.
        mlen: len = 5,
    }

    struct Key<'a> {
        /// The key.
        key: [u8; 16],
    }

    struct Uuids<'a> {
        /// The UUIDs.
        uuids: Array<'a, u32, Uuid>,
    }

    #[repr(u8)]
    enum QueryParameterType {
        #[default]
        Int = 1,
        Float = 2,
        String = 3,
        Uuid = 4,
    }
);

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    #[test]
    fn test_query() {
        let buf = QueryBuilder {
            query: "SELECT * from foo",
            types: &[QueryTypeBuilder {
                typ: QueryParameterType::Float,
                length: 4,
                meta: &[1, 2, 3, 4],
                ..Default::default()
            }],
            ..Default::default()
        }
        .to_vec();
        eprintln!("buf: {:?}", buf);
        let query = Query::new(&buf).expect("Failed to parse query");
        let types = query.types;
        assert_eq!(1, types.len());
        assert_eq!(
            r#"QueryType { typ: Float, len: 4, meta: [1, 2, 3, 4] }"#,
            format!("{:?}", types.into_iter().next().unwrap())
        );
        assert_eq!(
            r#"Query { mtype: 81, mlen: 37, query: "SELECT * from foo", types: [QueryType { typ: Float, len: 4, meta: [1, 2, 3, 4] }] }"#,
            format!("{query:?}")
        );
    }

    #[test]
    fn test_fixed_array() {
        let buf = KeyBuilder {
            key: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        }
        .to_vec();
        let key = Key::new(&buf).expect("Failed to parse key");
        assert_eq!(
            key.key,
            [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
        );
    }

    #[test]
    fn test_uuid() {
        let buf = UuidsBuilder {
            uuids: &[Uuid::NAMESPACE_DNS],
        }
        .to_vec();

        let uuids = Uuids::new(&buf).expect("Failed to parse uuids");
        assert_eq!(uuids.uuids.get(0), Some(Uuid::NAMESPACE_DNS));
    }

    #[test]
    fn test_fixed_length() {
        let buf = FixedLengthBuilder::default().to_vec();
        let fixed_length = FixedLength::new(&buf).expect("Failed to parse fixed length");
        assert_eq!(*fixed_length.mlen(), 5);
    }

    #[test]
    fn test_encoded() {
        let buf = DataRowBuilder {
            values: &[
                Encoded::Null,
                Encoded::Value(b"123"),
                Encoded::Null,
                Encoded::Value(b"456"),
            ],
        }
        .to_vec();
        eprintln!("buf: {:?}", buf);
        let data_row = DataRow::new(&buf).expect("Failed to parse data row");
        assert_eq!(data_row.values.len(), 4);
        let mut iter = data_row.values.into_iter();
        assert_eq!(iter.next().unwrap(), Encoded::Null);
        assert_eq!(iter.next().unwrap(), Encoded::Value(b"123"));
        assert_eq!(iter.next().unwrap(), Encoded::Null);
        assert_eq!(iter.next().unwrap(), Encoded::Value(b"456"));
    }
}
