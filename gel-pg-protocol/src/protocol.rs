use gel_db_protocol::{message_group, protocol};

message_group!(
    /// The `Backend` message group contains messages sent from the backend to the frontend.
    Backend: Message = [
        AuthenticationOk,
        AuthenticationKerberosV5,
        AuthenticationCleartextPassword,
        AuthenticationMD5Password,
        AuthenticationGSS,
        AuthenticationGSSContinue,
        AuthenticationSSPI,
        AuthenticationSASL,
        AuthenticationSASLContinue,
        AuthenticationSASLFinal,
        BackendKeyData,
        BindComplete,
        CloseComplete,
        CommandComplete,
        CopyData,
        CopyDone,
        CopyInResponse,
        CopyOutResponse,
        CopyBothResponse,
        DataRow,
        EmptyQueryResponse,
        ErrorResponse,
        FunctionCallResponse,
        NegotiateProtocolVersion,
        NoData,
        NoticeResponse,
        NotificationResponse,
        ParameterDescription,
        ParameterStatus,
        ParseComplete,
        PortalSuspended,
        ReadyForQuery,
        RowDescription
    ]
);

message_group!(
    /// The `Frontend` message group contains messages sent from the frontend to the backend.
    Frontend: Message = [
        Bind,
        Close,
        CopyData,
        CopyDone,
        CopyFail,
        Describe,
        Execute,
        Flush,
        FunctionCall,
        GSSResponse,
        Parse,
        PasswordMessage,
        Query,
        SASLInitialResponse,
        SASLResponse,
        Sync,
        Terminate
    ]
);

message_group!(
    /// The `Initial` message group contains messages that are sent before the
    /// normal message flow.
    Initial: InitialMessage = [
        CancelRequest,
        GSSENCRequest,
        SSLRequest,
        StartupMessage
    ]
);

protocol!(

/// A generic base for all Postgres mtype/mlen-style messages.
struct Message {
    /// Identifies the message.
    mtype: u8,
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// Message contents.
    data: Rest,
}

/// A generic base for all initial Postgres messages.
struct InitialMessage {
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// The identifier for this initial message.
    protocol_version: i32,
    /// Message contents.
    data: Rest
}

/// The `AuthenticationMessage` struct is a base for all Postgres authentication messages.
struct AuthenticationMessage: Message {
    /// Identifies the message as an authentication request.
    mtype: u8 = 'R',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// Specifies that the authentication was successful.
    status: i32,
}

/// The `AuthenticationOk` struct represents a message indicating successful authentication.
struct AuthenticationOk: Message {
    /// Identifies the message as an authentication request.
    mtype: u8 = 'R',
    /// Length of message contents in bytes, including self.
    mlen: len = 8,
    /// Specifies that the authentication was successful.
    status: i32 = 0,
}

/// The `AuthenticationKerberosV5` struct represents a message indicating that Kerberos V5 authentication is required.
struct AuthenticationKerberosV5: Message {
    /// Identifies the message as an authentication request.
    mtype: u8 = 'R',
    /// Length of message contents in bytes, including self.
    mlen: len = 8,
    /// Specifies that Kerberos V5 authentication is required.
    status: i32 = 2,
}

/// The `AuthenticationCleartextPassword` struct represents a message indicating that a cleartext password is required for authentication.
struct AuthenticationCleartextPassword: Message {
    /// Identifies the message as an authentication request.
    mtype: u8 = 'R',
    /// Length of message contents in bytes, including self.
    mlen: len = 8,
    /// Specifies that a clear-text password is required.
    status: i32 = 3,
}

/// The `AuthenticationMD5Password` struct represents a message indicating that an MD5-encrypted password is required for authentication.
struct AuthenticationMD5Password: Message {
    /// Identifies the message as an authentication request.
    mtype: u8 = 'R',
    /// Length of message contents in bytes, including self.
    mlen: len = 12,
    /// Specifies that an MD5-encrypted password is required.
    status: i32 = 5,
    /// The salt to use when encrypting the password.
    salt: [u8; 4],
}

/// The `AuthenticationSCMCredential` struct represents a message indicating that an SCM credential is required for authentication.
struct AuthenticationSCMCredential: Message {
    /// Identifies the message as an authentication request.
    mtype: u8 = 'R',
    /// Length of message contents in bytes, including self.
    mlen: len = 6,
    /// Any data byte, which is ignored.
    byte: u8 = 0,
}

/// The `AuthenticationGSS` struct represents a message indicating that GSSAPI authentication is required.
struct AuthenticationGSS: Message {
    /// Identifies the message as an authentication request.
    mtype: u8 = 'R',
    /// Length of message contents in bytes, including self.
    mlen: len = 8,
    /// Specifies that GSSAPI authentication is required.
    status: i32 = 7,
}

/// The `AuthenticationGSSContinue` struct represents a message indicating the continuation of GSSAPI authentication.
struct AuthenticationGSSContinue: Message {
    /// Identifies the message as an authentication request.
    mtype: u8 = 'R',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// Specifies that this message contains GSSAPI or SSPI data.
    status: i32 = 8,
    /// GSSAPI or SSPI authentication data.
    data: Rest,
}

/// The `AuthenticationSSPI` struct represents a message indicating that SSPI authentication is required.
struct AuthenticationSSPI: Message {
    /// Identifies the message as an authentication request.
    mtype: u8 = 'R',
    /// Length of message contents in bytes, including self.
    mlen: len = 8,
    /// Specifies that SSPI authentication is required.
    status: i32 = 9,
}

/// The `AuthenticationSASL` struct represents a message indicating that SASL authentication is required.
struct AuthenticationSASL: Message {
    /// Identifies the message as an authentication request.
    mtype: u8 = 'R',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// Specifies that SASL authentication is required.
    status: i32 = 10,
    /// List of SASL authentication mechanisms, terminated by a zero byte.
    mechanisms: ZTArray<ZTString>,
}

/// The `AuthenticationSASLContinue` struct represents a message containing a SASL challenge during the authentication process.
struct AuthenticationSASLContinue: Message {
    /// Identifies the message as an authentication request.
    mtype: u8 = 'R',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// Specifies that this message contains a SASL challenge.
    status: i32 = 11,
    /// SASL data, specific to the SASL mechanism being used.
    data: Rest,
}

/// The `AuthenticationSASLFinal` struct represents a message indicating the completion of SASL authentication.
struct AuthenticationSASLFinal: Message {
    /// Identifies the message as an authentication request.
    mtype: u8 = 'R',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// Specifies that SASL authentication has completed.
    status: i32 = 12,
    /// SASL outcome "additional data", specific to the SASL mechanism being used.
    data: Rest,
}

/// The `BackendKeyData` struct represents a message containing the process ID and secret key for this backend.
struct BackendKeyData: Message {
    /// Identifies the message as cancellation key data.
    mtype: u8 = 'K',
    /// Length of message contents in bytes, including self.
    mlen: len = 12,
    /// The process ID of this backend.
    pid: i32,
    /// The secret key of this backend.
    key: i32,
}

/// The `Bind` struct represents a message to bind a named portal to a prepared statement.
struct Bind: Message {
    /// Identifies the message as a Bind command.
    mtype: u8 = 'B',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// The name of the destination portal.
    portal: ZTString,
    /// The name of the source prepared statement.
    statement: ZTString,
    /// The parameter format codes.
    format_codes: Array<i16, i16>,
    /// Array of parameter values and their lengths.
    values: Array<i16, Encoded>,
    /// The result-column format codes.
    result_format_codes: Array<i16, i16>,
}

/// The `BindComplete` struct represents a message indicating that a Bind operation was successful.
struct BindComplete: Message {
    /// Identifies the message as a Bind-complete indicator.
    mtype: u8 = '2',
    /// Length of message contents in bytes, including self.
    mlen: len = 4,
}

/// The `CancelRequest` struct represents a message to request the cancellation of a query.
struct CancelRequest: InitialMessage {
    /// Length of message contents in bytes, including self.
    mlen: len = 16,
    /// The cancel request code.
    code: i32 = 80877102,
    /// The process ID of the target backend.
    pid: i32,
    /// The secret key for the target backend.
    key: i32,
}

/// The `Close` struct represents a message to close a prepared statement or portal.
struct Close: Message {
    /// Identifies the message as a Close command.
    mtype: u8 = 'C',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// 'S' to close a prepared statement; 'P' to close a portal.
    ctype: u8,
    /// The name of the prepared statement or portal to close.
    name: ZTString,
}

/// The `CloseComplete` struct represents a message indicating that a Close operation was successful.
struct CloseComplete: Message {
    /// Identifies the message as a Close-complete indicator.
    mtype: u8 = '3',
    /// Length of message contents in bytes, including self.
    mlen: len = 4,
}

/// The `CommandComplete` struct represents a message indicating the successful completion of a command.
struct CommandComplete: Message {
    /// Identifies the message as a command-completed response.
    mtype: u8 = 'C',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// The command tag.
    tag: ZTString,
}

/// The `CopyData` struct represents a message containing data for a copy operation.
struct CopyData: Message {
    /// Identifies the message as COPY data.
    mtype: u8 = 'd',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// Data that forms part of a COPY data stream.
    data: Rest,
}

/// The `CopyDone` struct represents a message indicating that a copy operation is complete.
struct CopyDone: Message {
    /// Identifies the message as a COPY-complete indicator.
    mtype: u8 = 'c',
    /// Length of message contents in bytes, including self.
    mlen: len = 4,
}

/// The `CopyFail` struct represents a message indicating that a copy operation has failed.
struct CopyFail: Message {
    /// Identifies the message as a COPY-failure indicator.
    mtype: u8 = 'f',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// An error message to report as the cause of failure.
    error_msg: ZTString,
}

/// The `CopyInResponse` struct represents a message indicating that the server is ready to receive data for a copy-in operation.
struct CopyInResponse: Message {
    /// Identifies the message as a Start Copy In response.
    mtype: u8 = 'G',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// 0 for textual, 1 for binary.
    format: u8,
    /// The format codes for each column.
    format_codes: Array<i16, i16>,
}

/// The `CopyOutResponse` struct represents a message indicating that the server is ready to send data for a copy-out operation.
struct CopyOutResponse: Message {
    /// Identifies the message as a Start Copy Out response.
    mtype: u8 = 'H',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// 0 for textual, 1 for binary.
    format: u8,
    /// The format codes for each column.
    format_codes: Array<i16, i16>,
}

/// The `CopyBothResponse` is used only for Streaming Replication.
struct CopyBothResponse: Message {
    /// Identifies the message as a Start Copy Both response.
    mtype: u8 = 'W',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// 0 for textual, 1 for binary.
    format: u8,
    /// The format codes for each column.
    format_codes: Array<i16, i16>,
}

/// The `DataRow` struct represents a message containing a row of data.
struct DataRow: Message {
    /// Identifies the message as a data row.
    mtype: u8 = 'D',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// Array of column values and their lengths.
    values: Array<i16, Encoded>,
}

/// The `Describe` struct represents a message to describe a prepared statement or portal.
struct Describe: Message {
    /// Identifies the message as a Describe command.
    mtype: u8 = 'D',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// 'S' to describe a prepared statement; 'P' to describe a portal.
    dtype: u8,
    /// The name of the prepared statement or portal.
    name: ZTString,
}

/// The `EmptyQueryResponse` struct represents a message indicating that an empty query string was recognized.
struct EmptyQueryResponse: Message {
    /// Identifies the message as a response to an empty query String.
    mtype: u8 = 'I',
    /// Length of message contents in bytes, including self.
    mlen: len = 4,
}

/// The `ErrorResponse` struct represents a message indicating that an error has occurred.
struct ErrorResponse: Message {
    /// Identifies the message as an error.
    mtype: u8 = 'E',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// Array of error fields and their values.
    fields: ZTArray<ErrorField>,
}

/// The `ErrorField` struct represents a single error message within an `ErrorResponse`.
struct ErrorField {
    /// A code identifying the field type.
    etype: u8,
    /// The field value.
    value: ZTString,
}

/// The `Execute` struct represents a message to execute a prepared statement or portal.
struct Execute: Message {
    /// Identifies the message as an Execute command.
    mtype: u8 = 'E',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// The name of the portal to execute.
    portal: ZTString,
    /// Maximum number of rows to return.
    max_rows: i32,
}

/// The `Flush` struct represents a message to flush the backend's output buffer.
struct Flush: Message {
    /// Identifies the message as a Flush command.
    mtype: u8 = 'H',
    /// Length of message contents in bytes, including self.
    mlen: len = 4,
}

/// The `FunctionCall` struct represents a message to call a function.
struct FunctionCall: Message {
    /// Identifies the message as a function call.
    mtype: u8 = 'F',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// OID of the function to execute.
    function_id: i32,
    /// The parameter format codes.
    format_codes: Array<i16, i16>,
    /// Array of args and their lengths.
    args: Array<i16, Encoded>,
    /// The format code for the result.
    result_format_code: i16,
}

/// The `FunctionCallResponse` struct represents a message containing the result of a function call.
struct FunctionCallResponse: Message {
    /// Identifies the message as a function-call response.
    mtype: u8 = 'V',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// The function result value.
    result: Encoded,
}

/// The `GSSENCRequest` struct represents a message requesting GSSAPI encryption.
struct GSSENCRequest: InitialMessage {
    /// Length of message contents in bytes, including self.
    mlen: len = 8,
    /// The GSSAPI Encryption request code.
    gssenc_request_code: i32 = 80877104,
}

/// The `GSSResponse` struct represents a message containing a GSSAPI or SSPI response.
struct GSSResponse: Message {
    /// Identifies the message as a GSSAPI or SSPI response.
    mtype: u8 = 'p',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// GSSAPI or SSPI authentication data.
    data: Rest,
}

/// The `NegotiateProtocolVersion` struct represents a message requesting protocol version negotiation.
struct NegotiateProtocolVersion: Message {
    /// Identifies the message as a protocol version negotiation request.
    mtype: u8 = 'v',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// Newest minor protocol version supported by the server.
    minor_version: i32,
    /// List of protocol options not recognized.
    options: Array<i32, ZTString>,
}

/// The `NoData` struct represents a message indicating that there is no data to return.
struct NoData: Message {
    /// Identifies the message as a No Data indicator.
    mtype: u8 = 'n',
    /// Length of message contents in bytes, including self.
    mlen: len = 4,
}

/// The `NoticeResponse` struct represents a message containing a notice.
struct NoticeResponse: Message {
    /// Identifies the message as a notice.
    mtype: u8 = 'N',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// Array of notice fields and their values.
    fields: ZTArray<NoticeField>,
}

/// The `NoticeField` struct represents a single error message within an `NoticeResponse`.
struct NoticeField: Message {
    /// A code identifying the field type.
    ntype: u8,
    /// The field value.
    value: ZTString,
}

/// The `NotificationResponse` struct represents a message containing a notification from the backend.
struct NotificationResponse: Message {
    /// Identifies the message as a notification.
    mtype: u8 = 'A',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// The process ID of the notifying backend.
    pid: i32,
    /// The name of the notification channel.
    channel: ZTString,
    /// The notification payload.
    payload: ZTString,
}

/// The `ParameterDescription` struct represents a message describing the parameters needed by a prepared statement.
struct ParameterDescription: Message {
    /// Identifies the message as a parameter description.
    mtype: u8 = 't',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// OIDs of the parameter data types.
    param_types: Array<i16, i32>,
}

/// The `ParameterStatus` struct represents a message containing the current status of a parameter.
struct ParameterStatus: Message {
    /// Identifies the message as a runtime parameter status report.
    mtype: u8 = 'S',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// The name of the parameter.
    name: ZTString,
    /// The current value of the parameter.
    value: ZTString,
}

/// The `Parse` struct represents a message to parse a query string.
struct Parse: Message {
    /// Identifies the message as a Parse command.
    mtype: u8 = 'P',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// The name of the destination prepared statement.
    statement: ZTString,
    /// The query string to be parsed.
    query: ZTString,
    /// OIDs of the parameter data types.
    param_types: Array<i16, i32>,
}

/// The `ParseComplete` struct represents a message indicating that a Parse operation was successful.
struct ParseComplete: Message {
    /// Identifies the message as a Parse-complete indicator.
    mtype: u8 = '1',
    /// Length of message contents in bytes, including self.
    mlen: len = 4,
}

/// The `PasswordMessage` struct represents a message containing a password.
struct PasswordMessage: Message {
    /// Identifies the message as a password response.
    mtype: u8 = 'p',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// The password (encrypted or plaintext, depending on context).
    password: ZTString,
}

/// The `PortalSuspended` struct represents a message indicating that a portal has been suspended.
struct PortalSuspended: Message {
    /// Identifies the message as a portal-suspended indicator.
    mtype: u8 = 's',
    /// Length of message contents in bytes, including self.
    mlen: len = 4,
}

/// The `Query` struct represents a message to execute a simple query.
struct Query: Message {
    /// Identifies the message as a simple query command.
    mtype: u8 = 'Q',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// The query String to be executed.
    query: ZTString,
}

/// The `ReadyForQuery` struct represents a message indicating that the backend is ready for a new query.
struct ReadyForQuery: Message {
    /// Identifies the message as a ready-for-query indicator.
    mtype: u8 = 'Z',
    /// Length of message contents in bytes, including self.
    mlen: len = 5,
    /// Current transaction status indicator.
    status: u8,
}

/// The `RowDescription` struct represents a message describing the rows that will be returned by a query.
struct RowDescription: Message {
    /// Identifies the message as a row description.
    mtype: u8 = 'T',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// Array of field descriptions.
    fields: Array<i16, RowField>,
}

/// The `RowField` struct represents a row within the `RowDescription` message.
struct RowField {
    /// The field name
    name: ZTString,
    /// The table ID (OID) of the table the column is from, or 0 if not a column reference
    table_oid: i32,
    /// The attribute number of the column, or 0 if not a column reference
    column_attr_number: i16,
    /// The object ID of the field's data type
    data_type_oid: i32,
    /// The data type size (negative if variable size)
    data_type_size: i16,
    /// The type modifier
    type_modifier: i32,
    /// The format code being used for the field (0 for text, 1 for binary)
    format_code: i16,
}

/// The `SASLInitialResponse` struct represents a message containing a SASL initial response.
struct SASLInitialResponse: Message {
    /// Identifies the message as a SASL initial response.
    mtype: u8 = 'p',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// Name of the SASL authentication mechanism.
    mechanism: ZTString,
    /// SASL initial response data.
    response: Array<i32, u8>,
}

/// The `SASLResponse` struct represents a message containing a SASL response.
struct SASLResponse: Message {
    /// Identifies the message as a SASL response.
    mtype: u8 = 'p',
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// SASL response data.
    response: Rest,
}

/// The `SSLRequest` struct represents a message requesting SSL encryption.
struct SSLRequest: InitialMessage {
    /// Length of message contents in bytes, including self.
    mlen: len = 8,
    /// The SSL request code.
    code: i32 = 80877103,
}

struct SSLResponse {
    /// Specifies if SSL was accepted or rejected.
    code: u8,
}

/// The `StartupMessage` struct represents a message to initiate a connection.
struct StartupMessage: InitialMessage {
    /// Length of message contents in bytes, including self.
    mlen: len,
    /// The protocol version number.
    protocol: i32 = 196608,
    /// List of parameter name-value pairs, terminated by a zero byte.
    params: ZTArray<StartupNameValue>,
}

/// The `StartupMessage` struct represents a name/value pair within the `StartupMessage` message.
struct StartupNameValue {
    /// The parameter name.
    name: ZTString,
    /// The parameter value.
    value: ZTString,
}

/// The `Sync` struct represents a message to synchronize the frontend and backend.
struct Sync: Message {
    /// Identifies the message as a Sync command.
    mtype: u8 = 'S',
    /// Length of message contents in bytes, including self.
    mlen: len = 4,
}

/// The `Terminate` struct represents a message to terminate a connection.
struct Terminate: Message {
    /// Identifies the message as a Terminate command.
    mtype: u8 = 'X',
    /// Length of message contents in bytes, including self.
    mlen: len = 4,
}
);

#[cfg(test)]
mod tests {
    use super::{builder, data::*, measure, meta};
    use gel_db_protocol::{match_message, Encoded, StructBuffer, StructMeta};
    use rand::Rng;

    /// We want to ensure that no malformed messages will cause unexpected
    /// panics, so we try all sorts of combinations of message mutation to
    /// ensure we don't.
    ///
    /// This isn't a 100% foolproof test.
    fn fuzz_test<S: StructMeta>(s: S::Struct<'_>) {
        let buf = S::to_vec(&s);

        // Re-create, won't panic
        fuzz_test_buf::<S>(&buf);

        // Truncating at any given length won't panic
        for i in 0..buf.len() {
            let mut buf = S::to_vec(&s);
            buf.truncate(i);
            fuzz_test_buf::<S>(&buf);
        }

        // Removing any particular value won't panic
        for i in 0..buf.len() {
            let mut buf = S::to_vec(&s);
            buf.remove(i);
            fuzz_test_buf::<S>(&buf);
        }

        // Zeroing any particular value won't panic
        for i in 0..buf.len() {
            let mut buf = S::to_vec(&s);
            buf[i] = 0;
            fuzz_test_buf::<S>(&buf);
        }

        // Corrupt each byte by incrementing (mod 256)
        for i in 0..buf.len() {
            let mut buf = S::to_vec(&s);
            buf[i] = buf[i].wrapping_add(1);
            fuzz_test_buf::<S>(&buf);
        }

        // Corrupt each byte by decrementing (mod 256)
        for i in 0..buf.len() {
            let mut buf = S::to_vec(&s);
            buf[i] = buf[i].wrapping_sub(1);
            fuzz_test_buf::<S>(&buf);
        }

        // Replace four-byte chunks at 1-byte offsets with "-2" in big-endian, one at a time
        // This shakes out any negative length issues for i32 lengths
        let negative_two_i32: i32 = -2;
        let bytes_i32 = negative_two_i32.to_be_bytes();
        for start_index in 0..buf.len().saturating_sub(3) {
            if start_index + 4 <= buf.len() {
                let mut buf = S::to_vec(&s); // Clean buffer for each iteration
                buf[start_index..start_index + 4].copy_from_slice(&bytes_i32);
                eprintln!("Replaced 4-byte chunk at offset {} with -2 (big-endian) in buffer of length {}", start_index, buf.len());
                fuzz_test_buf::<S>(&buf);
            }
        }

        // Replace two-byte chunks at 1-byte offsets with "-2" in big-endian, one at a time
        // This shakes out any negative length issues for i16 lengths
        let negative_two_i16: i16 = -2;
        let bytes_i16 = negative_two_i16.to_be_bytes();
        for start_index in 0..buf.len().saturating_sub(1) {
            if start_index + 2 <= buf.len() {
                let mut buf = S::to_vec(&s); // Clean buffer for each iteration
                buf[start_index..start_index + 2].copy_from_slice(&bytes_i16);
                eprintln!("Replaced 2-byte chunk at offset {} with -2 (big-endian) in buffer of length {}", start_index, buf.len());
                fuzz_test_buf::<S>(&buf);
            }
        }

        let run_count = if std::env::var("EXTENSIVE_FUZZ").is_ok() {
            100000
        } else {
            10
        };

        // Insert a random byte at a random position
        for i in 0..run_count {
            let mut buf = S::to_vec(&s);
            let random_byte: u8 = rand::rng().random();
            let random_position = rand::rng().random_range(0..=buf.len());
            buf.insert(random_position, random_byte);
            eprintln!(
                "Test {}: Inserted byte 0x{:02X} at position {} in buffer of length {}",
                i + 1,
                random_byte,
                random_position,
                buf.len()
            );
            fuzz_test_buf::<S>(&buf);
        }

        // Corrupt random parts of the buffer. This is non-deterministic.
        for i in 0..run_count {
            let mut buf = S::to_vec(&s);
            let rand: [u8; 4] = rand::rng().random();
            let n = rand::rng().random_range(0..buf.len() - 4);
            let range = n..n + 4;
            eprintln!(
                "Test {}: Corrupting buffer of length {} at range {:?} with bytes {:?}",
                i + 1,
                buf.len(),
                range,
                rand
            );
            buf.get_mut(range).unwrap().copy_from_slice(&rand);
            fuzz_test_buf::<S>(&buf);
        }

        // Corrupt 1..4 random bytes at random positions
        for i in 0..run_count {
            let mut buf = S::to_vec(&s);
            let num_bytes_to_corrupt = rand::rng().random_range(1..=4);
            let mut positions = Vec::new();

            for _ in 0..num_bytes_to_corrupt {
                let random_position = rand::rng().random_range(0..buf.len());
                if !positions.contains(&random_position) {
                    positions.push(random_position);
                    let random_byte: u8 = rand::rng().random();
                    buf[random_position] = random_byte;
                }
            }

            eprintln!(
                "Test {}: Corrupted {} byte(s) at position(s) {:?} in buffer of length {}",
                i + 1,
                positions.len(),
                positions,
                buf.len()
            );
            fuzz_test_buf::<S>(&buf);
        }

        // Attempt to parse randomly generated structs. This is non-deterministic.
        for i in 0..run_count {
            let buf: [u8; 16] = rand::rng().random();
            eprintln!(
                "Test {}: Attempting to parse random buffer: {:02X?}",
                i + 1,
                buf
            );
            fuzz_test_buf::<S>(&buf);
        }
    }

    fn fuzz_test_buf<S: StructMeta>(buf: &[u8]) {
        // Use std::fmt::Debug which will walk each field
        if let Ok(m) = S::new(buf) {
            let _ = format!("{:?}", m);
        }
    }

    #[test]
    fn test_sasl_response() {
        let buf = [b'p', 0, 0, 0, 5, 2];
        assert!(SASLResponse::is_buffer(&buf));
        let message = SASLResponse::new(&buf).unwrap();
        assert_eq!(message.mlen(), 5);
        assert_eq!(message.response().len(), 1);
    }

    #[test]
    fn test_sasl_response_measure() {
        let measure = measure::SASLResponse {
            response: &[1, 2, 3, 4, 5],
        };
        assert_eq!(measure.measure(), 10)
    }

    #[test]
    fn test_sasl_initial_response() {
        let buf = [
            b'p', 0, 0, 0, 0x36, // Mechanism
            b'S', b'C', b'R', b'A', b'M', b'-', b'S', b'H', b'A', b'-', b'2', b'5', b'6', 0,
            // Data
            0, 0, 0, 32, b'n', b',', b',', b'n', b'=', b',', b'r', b'=', b'p', b'E', b'k', b'P',
            b'L', b'Q', b'u', b'2', b'9', b'G', b'E', b'v', b'w', b'N', b'e', b'V', b'J', b't',
            b'7', b'2', b'a', b'r', b'Q', b'I',
        ];

        assert!(SASLInitialResponse::is_buffer(&buf));
        let message = SASLInitialResponse::new(&buf).unwrap();
        assert_eq!(message.mlen(), 0x36);
        assert_eq!(message.mechanism(), "SCRAM-SHA-256");
        assert_eq!(
            message.response().as_ref(),
            b"n,,n=,r=pEkPLQu29GEvwNeVJt72arQI"
        );

        fuzz_test::<meta::SASLInitialResponse>(message);
    }

    #[test]
    fn test_sasl_initial_response_builder() {
        let buf = builder::SASLInitialResponse {
            mechanism: "SCRAM-SHA-256",
            response: b"n,,n=,r=pEkPLQu29GEvwNeVJt72arQI",
        }
        .to_vec();

        let message = SASLInitialResponse::new(&buf).unwrap();
        assert_eq!(message.mlen(), 0x36);
        assert_eq!(message.mechanism(), "SCRAM-SHA-256");
        assert_eq!(
            message.response().as_ref(),
            b"n,,n=,r=pEkPLQu29GEvwNeVJt72arQI"
        );

        fuzz_test::<meta::SASLInitialResponse>(message);
    }

    #[test]
    fn test_startup_message() {
        let buf = [
            0, 0, 0, 41, 0, 0x03, 0, 0, 0x75, 0x73, 0x65, 0x72, 0, 0x70, 0x6f, 0x73, 0x74, 0x67,
            0x72, 0x65, 0x73, 0, 0x64, 0x61, 0x74, 0x61, 0x62, 0x61, 0x73, 0x65, 0, 0x70, 0x6f,
            0x73, 0x74, 0x67, 0x72, 0x65, 0x73, 0, 0,
        ];
        let message = StartupMessage::new(&buf).unwrap();
        assert_eq!(message.mlen(), buf.len());
        assert_eq!(message.protocol(), 196608);
        let arr = message.params();
        let mut vals = vec![];
        for entry in arr {
            vals.push(entry.name().to_owned().unwrap());
            vals.push(entry.value().to_owned().unwrap());
        }
        assert_eq!(vals, vec!["user", "postgres", "database", "postgres"]);

        fuzz_test::<meta::StartupMessage>(message);
    }

    #[test]
    fn test_row_description() {
        let buf = [
            b'T', 0, 0, 0, 48, // header
            0, 2, // # of fields
            b'f', b'1', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // field 1
            b'f', b'2', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // field 2
        ];
        assert!(RowDescription::is_buffer(&buf));
        let message = RowDescription::new(&buf).unwrap();
        assert_eq!(message.mlen(), buf.len() - 1);
        assert_eq!(message.fields().len(), 2);
        let mut iter = message.fields().into_iter();
        let f1 = iter.next().unwrap();
        assert_eq!(f1.name(), "f1");
        let f2 = iter.next().unwrap();
        assert_eq!(f2.name(), "f2");
        assert_eq!(None, iter.next());
        fuzz_test::<meta::RowDescription>(message);
    }

    #[test]
    fn test_row_description_measure() {
        let measure = measure::RowDescription {
            fields: &[
                measure::RowField { name: "F1" },
                measure::RowField { name: "F2" },
            ],
        };
        assert_eq!(49, measure.measure())
    }

    #[test]
    fn test_row_description_builder() {
        let builder = builder::RowDescription {
            fields: &[
                builder::RowField {
                    name: "F1",
                    column_attr_number: 1,
                    ..Default::default()
                },
                builder::RowField {
                    name: "F2",
                    data_type_oid: 1234,
                    format_code: 1,
                    ..Default::default()
                },
            ],
        };

        let vec = builder.to_vec();
        assert_eq!(49, vec.len());

        // Read it back
        assert!(RowDescription::is_buffer(&vec));
        let message = RowDescription::new(&vec).unwrap();
        assert_eq!(message.fields().len(), 2);
        let mut iter = message.fields().into_iter();
        let f1 = iter.next().unwrap();
        assert_eq!(f1.name(), "F1");
        assert_eq!(f1.column_attr_number(), 1);
        let f2 = iter.next().unwrap();
        assert_eq!(f2.name(), "F2");
        assert_eq!(f2.data_type_oid(), 1234);
        assert_eq!(f2.format_code(), 1);
        assert_eq!(None, iter.next());

        fuzz_test::<meta::RowDescription>(message);
    }

    #[test]
    fn test_message_polymorphism_sync() {
        let sync = builder::Sync::default();
        let buf = sync.to_vec();
        assert_eq!(buf.len(), 5);
        // Read it as a Message
        let message = Message::new(&buf).unwrap();
        assert_eq!(message.mlen(), 4);
        assert_eq!(message.mtype(), b'S');
        assert_eq!(message.data(), &[]);
        // And also a Sync
        assert!(Sync::is_buffer(&buf));
        let message = Sync::new(&buf).unwrap();
        assert_eq!(message.mlen(), 4);
        assert_eq!(message.mtype(), b'S');

        fuzz_test::<meta::Sync>(message);
    }

    #[test]
    fn test_message_polymorphism_rest() {
        let auth = builder::AuthenticationGSSContinue {
            data: &[1, 2, 3, 4, 5],
        };
        let buf = auth.to_vec();
        assert_eq!(14, buf.len());
        // Read it as a Message
        assert!(Message::is_buffer(&buf));
        let message = Message::new(&buf).unwrap();
        assert_eq!(message.mlen(), 13);
        assert_eq!(message.mtype(), b'R');
        assert_eq!(message.data(), &[0, 0, 0, 8, 1, 2, 3, 4, 5]);
        // And also a AuthenticationGSSContinue
        assert!(AuthenticationGSSContinue::is_buffer(&buf));
        let message = AuthenticationGSSContinue::new(&buf).unwrap();
        assert_eq!(message.mlen(), 13);
        assert_eq!(message.mtype(), b'R');
        assert_eq!(message.data(), &[1, 2, 3, 4, 5]);

        fuzz_test::<meta::AuthenticationGSSContinue>(message);
    }

    #[test]
    fn test_query_messages() {
        let data: Vec<u8> = vec![
            0x54, 0x00, 0x00, 0x00, 0x21, 0x00, 0x01, 0x3f, b'c', b'o', b'l', b'u', b'm', b'n',
            0x3f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x17, 0x00, 0x04,
            0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x44, 0x00, 0x00, 0x00, 0x0b, 0x00, 0x01, 0x00,
            0x00, 0x00, 0x01, b'1', b'C', 0x00, 0x00, 0x00, 0x0d, b'S', b'E', b'L', b'E', b'C',
            b'T', b' ', b'1', 0x00, 0x5a, 0x00, 0x00, 0x00, 0x05, b'I',
        ];

        let mut buffer = StructBuffer::<meta::Message>::default();
        buffer.push(&data, |message| {
            match_message!(message, Backend {
                (RowDescription as row) => {
                    assert_eq!(row.fields().len(), 1);
                    let field = row.fields().into_iter().next().unwrap();
                    assert_eq!(field.name(), "?column?");
                    assert_eq!(field.data_type_oid(), 23);
                    assert_eq!(field.format_code(), 0);
                    eprintln!("{row:?}");
                    fuzz_test::<meta::RowDescription>(row);
                },
                (DataRow as row) => {
                    assert_eq!(row.values().len(), 1);
                    assert_eq!(row.values().into_iter().next().unwrap(), "1");
                    eprintln!("{row:?}");
                    fuzz_test::<meta::DataRow>(row);
                },
                (CommandComplete as complete) => {
                    assert_eq!(complete.tag(), "SELECT 1");
                    eprintln!("{complete:?}");
                },
                (ReadyForQuery as ready) => {
                    assert_eq!(ready.status(), b'I');
                    eprintln!("{ready:?}");
                },
                unknown => {
                    panic!("Unknown message type: {:?}", unknown);
                }
            });
        });
    }

    #[test]
    fn test_encode_data_row() {
        builder::DataRow {
            values: &[Encoded::Value(b"1")],
        }
        .to_vec();
    }

    #[test]
    fn test_parse() {
        let buf = [
            b'P', // message type
            0, 0, 0, 25, // message length
            b'S', b't', b'm', b't', 0, // statement name
            b'S', b'E', b'L', b'E', b'C', b'T', b' ', b'$', b'1', 0, // query string
            0, 1, // number of parameter data types
            0, 0, 0, 23, // OID
        ];

        assert!(Parse::is_buffer(&buf));
        let message = Parse::new(&buf).unwrap();
        assert_eq!(message.mlen(), 25);
        assert_eq!(message.statement(), "Stmt");
        assert_eq!(message.query(), "SELECT $1");
        assert_eq!(message.param_types().len(), 1);
        assert_eq!(message.param_types().get(0).unwrap(), 23); // OID

        fuzz_test::<meta::Parse>(message);
    }

    #[test]
    fn test_function_call() {
        let buf = builder::FunctionCall {
            function_id: 100,
            format_codes: &[0],
            args: &[Encoded::Value(b"123")],
            result_format_code: 0,
        }
        .to_vec();

        assert!(FunctionCall::is_buffer(&buf));
        let message = FunctionCall::new(&buf).unwrap();
        assert_eq!(message.function_id(), 100);
        assert_eq!(message.format_codes().len(), 1);
        assert_eq!(message.format_codes().get(0).unwrap(), 0);
        assert_eq!(message.args().len(), 1);
        assert_eq!(
            message.args().into_iter().next().unwrap(),
            b"123".as_slice()
        );
        assert_eq!(message.result_format_code(), 0);

        fuzz_test::<meta::FunctionCall>(message);
    }

    #[test]
    fn test_datarow() {
        let buf = [
            0x44, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x01, 0xff, 0xff, 0xff, 0xff,
        ];
        assert!(DataRow::is_buffer(&buf));
        let message = DataRow::new(&buf).unwrap();
        assert_eq!(message.values().len(), 1);
        assert_eq!(message.values().into_iter().next().unwrap(), Encoded::Null);
    }
}
