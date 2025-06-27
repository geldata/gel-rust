use tracing::{error, trace};

use crate::{
    config::ListenerConfig,
    stream::{ListenerStream, StreamProperties, TransportType},
};

pub(crate) const PREFACE_SIZE: usize = 8;
pub(crate) const MIN_PREFACE_SIZE: usize = 5;

const HTTP_2: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
const HTTP_2_PREFACE: [u8; PREFACE_SIZE] = [
    HTTP_2[0], HTTP_2[1], HTTP_2[2], HTTP_2[3], HTTP_2[4], HTTP_2[5], HTTP_2[6], HTTP_2[7],
];

#[derive(Clone, Copy, Debug)]
pub enum PostgresInitialMessage {
    StartupMessage,
    SSLRequest,
    GSSENCRequest,
    Cancellation,
}

#[derive(Clone, Copy, Debug)]
pub enum StreamState {
    /// A stream that is in the raw state.
    Raw,
    /// A stream that began in SSL.
    Ssl,
    /// A stream that has gone through the Postgres SSL upgrade handshake (whether successful or not).
    PgSslUpgrade,
    /// A stream encapsulated within another stream (ie: WebSocket/HTTP)
    Encapsulated,
}

/// Represents the different types of streams that can be identified.
#[derive(Clone, Copy, Debug)]
pub enum StreamType {
    /// PostgreSQL initial messages.
    PostgresInitial(PostgresInitialMessage),
    /// SSL/TLS connection.
    SSLTLS,
    /// Gel/EdgeDB binary protocol.
    GelBinary,
    /// HTTP/2 protocol.
    HTTP2,
    /// HTTP/1.x protocols.
    HTTP1x,
}

impl StreamType {
    pub fn go_away_message(&self) -> &'static [u8] {
        match self {
            StreamType::PostgresInitial(_) => {
                b"E\0\0\0\x3dSFATAL\0VFATAL\0C0A000\0MPostgreSQL protocol not supported\0\0"
            }
            StreamType::SSLTLS => b"\x15\x03\x00\x00\x02\x02\x46", // TLS Alert: Protocol Version (0x46) - Handshake Failure
            StreamType::GelBinary => {
                // FATAL error response for EdgeDB binary protocol
                &[
                    0x45, // 'E'
                    0x00, 0x00, 0x00, 13,   // Message length
                    0xC8, // Severity: FATAL
                    0x0A, 0x00, 0x00, 0x00, // Error code: Protocol error
                    0x45, 0x00, // Null-terminated message
                    0x00, 0x00, // Number of attributes (0)
                ]
            }
            StreamType::HTTP2 => &[
                // SETTINGS frame
                0x00, 0x00, 0x00, // Length (0 for empty SETTINGS)
                0x04, // Type (SETTINGS)
                0x00, // Flags
                0x00, 0x00, 0x00, 0x00, // Stream Identifier
                // GOAWAY frame
                0x00, 0x00, 0x08, // Length
                0x07, // Type (GOAWAY)
                0x00, // Flags
                0x00, 0x00, 0x00, 0x00, // Stream Identifier
                0x00, 0x00, 0x00, 0x00, // Last-Stream-ID
                0x00, 0x00, 0x00, 0x0d, // Error Code (PROTOCOL_ERROR)
            ],
            StreamType::HTTP1x => {
                b"HTTP/1.1 505 HTTP Version Not Supported\r\nContent-Length: 0\r\n\r\n"
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum UnknownStreamType {
    Unknown,
    PostgresInitial,
    SSLLegacy,
    PostgresWireLegacy,
}

impl UnknownStreamType {
    pub fn go_away_message(&self) -> &'static [u8] {
        match self {
            UnknownStreamType::Unknown => b"Invalid startup message",
            UnknownStreamType::PostgresInitial => {
                b"E\0\0\0\x89SFATAL\0VFATAL\0C0A000\0Munsupported startup message\0\0"
            }
            // This response results in an OpenSSL error of "no cipher" which is
            // a decent way to respond.
            // "SSL routines:GET_SERVER_HELLO:peer error no cipher:s2_pkt.c:667:"
            UnknownStreamType::SSLLegacy => b"\x80\x03\0\0\x01",
            // This is sufficient to make any PostgreSQL version go away
            UnknownStreamType::PostgresWireLegacy => b"EInvalid protocol\0",
        }
    }
}

const fn is_likely_http1(a1: u8, a2: u8, a3: u8, a4: u8) -> bool {
    a1.is_ascii_uppercase()
        && a2.is_ascii_uppercase()
        && a3.is_ascii_uppercase()
        && (a4.is_ascii_uppercase() || a4.is_ascii_whitespace())
}

const fn identify_connection(
    state: StreamState,
    buf: &[u8; PREFACE_SIZE],
) -> Result<StreamType, UnknownStreamType> {
    match (state, buf) {
        // Legacy Postgres wire protocol (Network order length = 296, major version = 2)
        // Example connection from PostgreSQL 7.x:
        // 00000000  00 00 01 28 00 02 00 00  6d 61 74 74 00 00 00 00  |...(....matt....|
        (_, [0, 0, 0x01, 0x28, 0, 2, _, _]) => Err(UnknownStreamType::PostgresWireLegacy),

        // SSL 2: Record type 0x01 (ClientHello)
        // Example connection from openssl 1.0.2k with `-ssl2`:
        // 00000000  80 25 01 00 02 00 0c 00  00 00 10 05 00 80 03 00  |.%..............|
        // 00000010  80 01 00 80 07 00 c0 37  9b dc 4b 94 2c ba 14 57  |.......7..K.,..W|
        (StreamState::Raw, [b0, b1, 0x01, ..]) if (*b0 & 0x80 == 0x80) && ((((*b0 as u16 & 0x7f) << 8) | *b1 as u16) > 9) => Err(UnknownStreamType::SSLLegacy),

        // Postgres wire protocol: Startup message with length 13 or more, protocol = 0x30000
        (_, [0, 0, len_hi, len_lo, 0, 3, 0, 0]) if u32::from_be_bytes([0, 0, *len_hi, *len_lo]) >= 13 => Ok(StreamType::PostgresInitial(PostgresInitialMessage::StartupMessage)),

        // Postgres SSLRequest startup message (length 8, code 0x4d2162f)
        (StreamState::Raw, [0, 0, 0, 8, 0x04, 0xd2, 0x16, 0x2f]) => Ok(StreamType::PostgresInitial(PostgresInitialMessage::SSLRequest)),

        // Postgres GSSENCRequest startup message (length 8, code 0x4d21630)
        (_, [0, 0, 0, 8, 0x04, 0xd2, 0x16, 0x30]) => Ok(StreamType::PostgresInitial(PostgresInitialMessage::GSSENCRequest)),

        // Other Postgres startup message (length 8 or 16, code 0x4d2....)
        (_, [0, 0, 0, 8 | 16, 0x04, 0xd2, _, _]) => Ok(StreamType::PostgresInitial(PostgresInitialMessage::Cancellation)),

        // Other Postgres startup message (code 0x4d216??)
        (_, [0, 0, _, _, 0x04, 0xd2, 0x16, _]) => Err(UnknownStreamType::PostgresInitial),

        // SSL 3.0 or TLS 1.0+: Record type 0x16 (Handshake), Version 3.0 or higher
        (StreamState::Raw, [0x16, 0x03, _, _, _, 0x01, ..]) => Ok(StreamType::SSLTLS),

        // EdgeDB binary protocol (ClientHandshake): 'V' followed by 7 bytes (including length)
        (StreamState::Raw | StreamState::Ssl | StreamState::Encapsulated, [b'V', 0, 0, 0, _, _, _, _]) => Ok(StreamType::GelBinary),

        // HTTP/2: Connection Preface
        (StreamState::Raw | StreamState::Ssl, &HTTP_2_PREFACE) => Ok(StreamType::HTTP2),

        // HTTP/1.x: Various HTTP methods

        // GET /<*>
        (StreamState::Raw | StreamState::Ssl, [b'G', b'E', b'T', b' ', b'/', ..] |
        // POST /<*>
        [b'P', b'O', b'S', b'T', b' ', b'/', ..] |
        // PUT /<*>
        [b'P', b'U', b'T', b' ', b'/', ..] |
        // DELETE /<*>
        [b'D', b'E', b'L', b'E', b'T', b'E', b' ', ..] |
        // HEAD /<*>
        [b'H', b'E', b'A', b'D', b' ', b'/', ..] |
        // OPTIONS /<*> or OPTIONS *
        [b'O', b'P', b'T', b'I', b'O', b'N', b'S', ..] |
        // PATCH /<*>
        [b'P', b'A', b'T', b'C', b'H', b' ', b'/', ..] |
        // TRACE /<*>
        [b'T', b'R', b'A', b'C', b'E', b' ', b'/', ..] |
        // CONNECT <*>
        [b'C', b'O', b'N', b'N', b'E', b'C', b'T', b' ']) => Ok(StreamType::HTTP1x),
        // Other less common HTTP methods, assume HTTP/1.x if the first
        // four bytes look like an HTTP method
        (StreamState::Raw | StreamState::Ssl, [a1, a2, a3, a4, ..]) if is_likely_http1(*a1, *a2, *a3, *a4) => Ok(StreamType::HTTP1x),

        // Unknown protocol
        _ => Err(UnknownStreamType::Unknown),
    }
}

pub const STR_EDGEDB_BINARY: &str = "edgedb-binary";
pub const STR_GEL_BINARY: &str = "gel-binary";
pub const STR_POSTGRESQL: &str = "postgresql";
pub const STR_HTTP2: &str = "h2";
pub const STR_HTTP1_1: &str = "http/1.1";
pub const ALPN_EDGEDB_BINARY: &[u8] = STR_EDGEDB_BINARY.as_bytes();
pub const ALPN_GEL_BINARY: &[u8] = STR_GEL_BINARY.as_bytes();
pub const ALPN_POSTGRESQL: &[u8] = STR_POSTGRESQL.as_bytes();
pub const ALPN_HTTP2: &[u8] = STR_HTTP2.as_bytes();
pub const ALPN_HTTP1_1: &[u8] = STR_HTTP1_1.as_bytes();

pub fn negotiate_alpn(
    config: &impl ListenerConfig,
    alpn: &[u8],
    stream_props: &StreamProperties,
) -> Option<&'static str> {
    let mut i = 0;
    let mut edgedb_binary = false;
    let mut postgresql = false;
    let mut h2 = false;
    let mut http1_1 = false;

    while i < alpn.len() {
        let len = alpn[i] as usize;
        if i + 1 + len > alpn.len() {
            break;
        }
        let protocol = &alpn[i + 1..i + 1 + len];
        match protocol {
            ALPN_EDGEDB_BINARY => {
                edgedb_binary = true;
            }
            ALPN_POSTGRESQL => {
                postgresql = true;
            }
            ALPN_HTTP2 => {
                h2 = true;
            }
            ALPN_HTTP1_1 => {
                http1_1 = true;
            }
            _ => {}
        }
        i += 1 + len;
    }

    if edgedb_binary
        && config
            .is_supported(
                Some(StreamType::GelBinary),
                TransportType::Ssl,
                stream_props,
            )
            .is_yes_or_maybe()
    {
        Some(STR_EDGEDB_BINARY)
    } else if postgresql
        && config
            .is_supported(
                Some(StreamType::PostgresInitial(
                    PostgresInitialMessage::StartupMessage,
                )),
                TransportType::Ssl,
                stream_props,
            )
            .is_yes_or_maybe()
    {
        Some(STR_POSTGRESQL)
    } else if h2
        && config
            .is_supported(Some(StreamType::HTTP2), TransportType::Ssl, stream_props)
            .is_yes_or_maybe()
    {
        Some(STR_HTTP2)
    } else if http1_1
        && config
            .is_supported(Some(StreamType::HTTP1x), TransportType::Ssl, stream_props)
            .is_yes_or_maybe()
    {
        Some(STR_HTTP1_1)
    } else {
        error!("No supported ALPN protocol found");
        None
    }
}

pub fn negotiate_ws_protocol(
    config: &impl ListenerConfig,
    protocols: &str,
    stream_props: &StreamProperties,
) -> Option<&'static str> {
    let mut edgedb_binary = false;
    let mut postgresql = false;

    for protocol in protocols.split(',') {
        match protocol.trim() {
            STR_EDGEDB_BINARY => {
                edgedb_binary = true;
            }
            STR_POSTGRESQL => {
                postgresql = true;
            }
            _ => {}
        }
    }

    if edgedb_binary
        && config
            .is_supported(
                Some(StreamType::GelBinary),
                TransportType::WebSocket,
                stream_props,
            )
            .is_yes_or_maybe()
    {
        Some(STR_EDGEDB_BINARY)
    } else if postgresql
        && config
            .is_supported(
                Some(StreamType::PostgresInitial(
                    PostgresInitialMessage::StartupMessage,
                )),
                TransportType::WebSocket,
                stream_props,
            )
            .is_yes_or_maybe()
    {
        Some(STR_POSTGRESQL)
    } else {
        error!("No supported WebSocket protocol found");
        None
    }
}

/// Identifies the ALPN protocol and returns the corresponding StreamType
fn identify_alpn(alpn: &[u8]) -> Result<StreamType, UnknownStreamType> {
    match alpn {
        ALPN_EDGEDB_BINARY => Ok(StreamType::GelBinary),
        ALPN_POSTGRESQL => Ok(StreamType::PostgresInitial(
            PostgresInitialMessage::StartupMessage,
        )),
        ALPN_HTTP2 => Ok(StreamType::HTTP2),
        ALPN_HTTP1_1 => Ok(StreamType::HTTP1x),
        _ => Err(UnknownStreamType::Unknown),
    }
}

/// Maps a byte slice to the corresponding ALPN protocol string.
pub fn known_protocol(alpn: &[u8]) -> Option<&'static str> {
    match alpn {
        ALPN_EDGEDB_BINARY => Some(STR_EDGEDB_BINARY),
        ALPN_POSTGRESQL => Some(STR_POSTGRESQL),
        ALPN_HTTP2 => Some(STR_HTTP2),
        ALPN_HTTP1_1 => Some(STR_HTTP1_1),
        _ => None,
    }
}

/// Identifies the stream type based on the initial bytes read from the socket or ALPN protocol.
/// This function correctly rewinds the stream if needed.
pub async fn identify_stream(
    state: StreamState,
    socket: &mut ListenerStream,
) -> Result<StreamType, UnknownStreamType> {
    // If we have a negotiated ALPN/websocket type, we don't need to sniff the stream
    if let Some(alpn) = socket.selected_protocol() {
        let res = identify_alpn(alpn.as_bytes());
        trace!("Identified connection via ALPN/websocket: {alpn:?} -> {res:?}");
        return res;
    }

    // TODO: Should add a custom preface sniffer for gel-stream so we can bail with "GET /"
    if let Some(preface) = socket.preface() {
        let res = identify_connection(state, &preface);
        trace!(
            "Identified connection via preface: {:?} -> {res:?}",
            preface
        );
        res
    } else {
        Err(UnknownStreamType::Unknown)
    }
}
