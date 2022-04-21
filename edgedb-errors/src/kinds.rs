use crate::error::Tag;
use crate::traits::{ErrorKind, Sealed};

macro_rules! define_errors {
    ($( (struct $id:ident, $code:expr, $tags: expr), )*) => {
        $(
            pub struct $id;

            impl Sealed for $id {
                const CODE: u32 = $code;
                const NAME: &'static str = stringify!($id);
                const TAGS: u32 = $tags;
            }

            impl ErrorKind for $id {}
        )*
        pub(crate) fn tag_check(code: u32, bit: u32) -> bool {
            let tag_mask = match code {
                $(
                    $code => $tags,
                )*
                _ => 0,
            };
            return tag_mask & (1 << bit) != 0;
        }
        pub(crate) fn error_name(code: u32) -> &'static str {
            match code {
                $(
                    $code => stringify!($id),
                )*
                _ => "EdgeDBError",
            }
        }
    }
}

// AUTOGENERATED BY EdgeDB WITH
//     $ cargo run --bin edgedb_gen_errors -- errors.txt

#[allow(unused_macros)]  // fake macro for generator
macro_rules! define_tag {
    ($name: ident, $bit: expr) => {
        pub static $name: Tag = Tag { bit: $bit };
    }
}

// <define_tag>
pub static SHOULD_RECONNECT: Tag = Tag { bit: 0 };
pub static SHOULD_RETRY: Tag = Tag { bit: 1 };
// </define_tag>

#[allow(unused_macros)]  // fake macro for generator
macro_rules! define_error {
    (struct $name: ident, $code: expr, $tag_bits: expr) => {
        (struct $name, $code, $tag_bits),
    }
}

define_errors![
    // <define_error>
    (struct InternalServerError, 0x01000000u32, 0x00000000),
    (struct UnsupportedFeatureError, 0x02000000u32, 0x00000000),
    (struct ProtocolError, 0x03000000u32, 0x00000000),
    (struct BinaryProtocolError, 0x03010000u32, 0x00000000),
    (struct UnsupportedProtocolVersionError, 0x03010001u32, 0x00000000),
    (struct TypeSpecNotFoundError, 0x03010002u32, 0x00000000),
    (struct UnexpectedMessageError, 0x03010003u32, 0x00000000),
    (struct InputDataError, 0x03020000u32, 0x00000000),
    (struct ResultCardinalityMismatchError, 0x03030000u32, 0x00000000),
    (struct CapabilityError, 0x03040000u32, 0x00000000),
    (struct UnsupportedCapabilityError, 0x03040100u32, 0x00000000),
    (struct DisabledCapabilityError, 0x03040200u32, 0x00000000),
    (struct QueryError, 0x04000000u32, 0x00000000),
    (struct InvalidSyntaxError, 0x04010000u32, 0x00000000),
    (struct EdgeQLSyntaxError, 0x04010100u32, 0x00000000),
    (struct SchemaSyntaxError, 0x04010200u32, 0x00000000),
    (struct GraphQLSyntaxError, 0x04010300u32, 0x00000000),
    (struct InvalidTypeError, 0x04020000u32, 0x00000000),
    (struct InvalidTargetError, 0x04020100u32, 0x00000000),
    (struct InvalidLinkTargetError, 0x04020101u32, 0x00000000),
    (struct InvalidPropertyTargetError, 0x04020102u32, 0x00000000),
    (struct InvalidReferenceError, 0x04030000u32, 0x00000000),
    (struct UnknownModuleError, 0x04030001u32, 0x00000000),
    (struct UnknownLinkError, 0x04030002u32, 0x00000000),
    (struct UnknownPropertyError, 0x04030003u32, 0x00000000),
    (struct UnknownUserError, 0x04030004u32, 0x00000000),
    (struct UnknownDatabaseError, 0x04030005u32, 0x00000000),
    (struct UnknownParameterError, 0x04030006u32, 0x00000000),
    (struct SchemaError, 0x04040000u32, 0x00000000),
    (struct SchemaDefinitionError, 0x04050000u32, 0x00000000),
    (struct InvalidDefinitionError, 0x04050100u32, 0x00000000),
    (struct InvalidModuleDefinitionError, 0x04050101u32, 0x00000000),
    (struct InvalidLinkDefinitionError, 0x04050102u32, 0x00000000),
    (struct InvalidPropertyDefinitionError, 0x04050103u32, 0x00000000),
    (struct InvalidUserDefinitionError, 0x04050104u32, 0x00000000),
    (struct InvalidDatabaseDefinitionError, 0x04050105u32, 0x00000000),
    (struct InvalidOperatorDefinitionError, 0x04050106u32, 0x00000000),
    (struct InvalidAliasDefinitionError, 0x04050107u32, 0x00000000),
    (struct InvalidFunctionDefinitionError, 0x04050108u32, 0x00000000),
    (struct InvalidConstraintDefinitionError, 0x04050109u32, 0x00000000),
    (struct InvalidCastDefinitionError, 0x0405010Au32, 0x00000000),
    (struct DuplicateDefinitionError, 0x04050200u32, 0x00000000),
    (struct DuplicateModuleDefinitionError, 0x04050201u32, 0x00000000),
    (struct DuplicateLinkDefinitionError, 0x04050202u32, 0x00000000),
    (struct DuplicatePropertyDefinitionError, 0x04050203u32, 0x00000000),
    (struct DuplicateUserDefinitionError, 0x04050204u32, 0x00000000),
    (struct DuplicateDatabaseDefinitionError, 0x04050205u32, 0x00000000),
    (struct DuplicateOperatorDefinitionError, 0x04050206u32, 0x00000000),
    (struct DuplicateViewDefinitionError, 0x04050207u32, 0x00000000),
    (struct DuplicateFunctionDefinitionError, 0x04050208u32, 0x00000000),
    (struct DuplicateConstraintDefinitionError, 0x04050209u32, 0x00000000),
    (struct DuplicateCastDefinitionError, 0x0405020Au32, 0x00000000),
    (struct SessionTimeoutError, 0x04060000u32, 0x00000000),
    (struct IdleSessionTimeoutError, 0x04060100u32, 0x00000000),
    (struct QueryTimeoutError, 0x04060200u32, 0x00000000),
    (struct TransactionTimeoutError, 0x04060A00u32, 0x00000000),
    (struct IdleTransactionTimeoutError, 0x04060A01u32, 0x00000000),
    (struct ExecutionError, 0x05000000u32, 0x00000000),
    (struct InvalidValueError, 0x05010000u32, 0x00000000),
    (struct DivisionByZeroError, 0x05010001u32, 0x00000000),
    (struct NumericOutOfRangeError, 0x05010002u32, 0x00000000),
    (struct IntegrityError, 0x05020000u32, 0x00000000),
    (struct ConstraintViolationError, 0x05020001u32, 0x00000000),
    (struct CardinalityViolationError, 0x05020002u32, 0x00000000),
    (struct MissingRequiredError, 0x05020003u32, 0x00000000),
    (struct TransactionError, 0x05030000u32, 0x00000000),
    (struct TransactionConflictError, 0x05030100u32, 0x00000002),
    (struct TransactionSerializationError, 0x05030101u32, 0x00000002),
    (struct TransactionDeadlockError, 0x05030102u32, 0x00000002),
    (struct ConfigurationError, 0x06000000u32, 0x00000000),
    (struct AccessError, 0x07000000u32, 0x00000000),
    (struct AuthenticationError, 0x07010000u32, 0x00000000),
    (struct AvailabilityError, 0x08000000u32, 0x00000000),
    (struct BackendUnavailableError, 0x08000001u32, 0x00000002),
    (struct BackendError, 0x09000000u32, 0x00000000),
    (struct UnsupportedBackendFeatureError, 0x09000100u32, 0x00000000),
    (struct LogMessage, 0xF0000000u32, 0x00000000),
    (struct WarningMessage, 0xF0010000u32, 0x00000000),
    (struct ClientError, 0xFF000000u32, 0x00000000),
    (struct ClientConnectionError, 0xFF010000u32, 0x00000000),
    (struct ClientConnectionFailedError, 0xFF010100u32, 0x00000000),
    (struct ClientConnectionFailedTemporarilyError, 0xFF010101u32, 0x00000003),
    (struct ClientConnectionTimeoutError, 0xFF010200u32, 0x00000003),
    (struct ClientConnectionClosedError, 0xFF010300u32, 0x00000003),
    (struct InterfaceError, 0xFF020000u32, 0x00000000),
    (struct QueryArgumentError, 0xFF020100u32, 0x00000000),
    (struct MissingArgumentError, 0xFF020101u32, 0x00000000),
    (struct UnknownArgumentError, 0xFF020102u32, 0x00000000),
    (struct InvalidArgumentError, 0xFF020103u32, 0x00000000),
    (struct NoDataError, 0xFF030000u32, 0x00000000),
    (struct InternalClientError, 0xFF040000u32, 0x00000000),
    // </define_error>
    (struct ProtocolTlsError, 0x03FF0000u32, 0x00000000),
    (struct ProtocolOutOfOrderError, 0x03FE0000u32, 0x00000000),
    (struct ProtocolEncodingError, 0x03FD0000u32, 0x00000000),
    (struct PasswordRequired, 0x0701FF00u32, 0x00000000),
    (struct ClientInconsistentError, 0xFFFF0000u32, 0x00000000),
    (struct ClientEncodingError, 0xFFFE0000u32, 0x00000000),
    (struct ClientNoCredentialsError, 0xFF0101FFu32, 0x00000000),
    (struct ClientConnectionEosError, 0xFF01FF00u32, 0x00000000),
    (struct NoResultExpected, 0xFF02FF00u32, 0x00000000),
    (struct DescriptorMismatch, 0xFF02FE00u32, 0x00000000),
    (struct UserError, 0xFE000000u32, 0x00000000),
];
