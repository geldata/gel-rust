#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtocolVersion {
    pub(crate) major_ver: u16,
    pub(crate) minor_ver: u16,
}

impl ProtocolVersion {
    pub fn current() -> ProtocolVersion {
        ProtocolVersion {
            major_ver: 3,
            minor_ver: 0,
        }
    }
    pub fn new(major_ver: u16, minor_ver: u16) -> ProtocolVersion {
        debug_assert!(
            major_ver >= 2,
            "Attempted to create a protocol version less than 2"
        );
        ProtocolVersion {
            major_ver,
            minor_ver,
        }
    }
    pub fn version_tuple(&self) -> (u16, u16) {
        (self.major_ver, self.minor_ver)
    }
    pub fn is_3(&self) -> bool {
        self.major_ver >= 3
    }
    pub fn is_multilingual(&self) -> bool {
        self.is_at_least(3, 0)
    }
    pub fn is_at_least(&self, major_ver: u16, minor_ver: u16) -> bool {
        debug_assert!(
            major_ver >= 3,
            "Attempted to compare protocol version less than 3"
        );
        self.major_ver > major_ver || self.major_ver == major_ver && self.minor_ver >= minor_ver
    }
    pub fn is_at_most(&self, major_ver: u16, minor_ver: u16) -> bool {
        debug_assert!(
            major_ver >= 3,
            "Attempted to compare protocol version less than 3"
        );
        self.major_ver < major_ver || self.major_ver == major_ver && self.minor_ver <= minor_ver
    }
}
