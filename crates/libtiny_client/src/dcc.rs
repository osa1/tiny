use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::num::ParseIntError;
use std::str::FromStr;

#[derive(Debug, PartialEq)]
pub(crate) struct DccRecord {
    /// SEND or CHAT (only supporting SEND right now)
    dcc_type: DccType,
    /// IP address and port
    address: SocketAddr,
    /// Nickname of person who wants to send
    origin: String,
    /// Client nickname
    receiver: String,
    /// Argument - filename or string "chat"
    argument: String,
    /// File size of file that will be sent in bytes
    file_size: Option<u32>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum DccType {
    SEND,
    CHAT,
}

#[derive(Debug)]
pub struct DccRecordInfo {
    /// SEND or CHAT (only supporting SEND right now)
    pub dcc_type: DccType,
    /// Argument - filename or string "chat"
    pub argument: String,
    /// File size of file that will be sent in bytes
    pub file_size: Option<u32>,
}

#[derive(Debug)]
pub enum DccParseError {
    DccType,
    DccRecord,
}

impl std::error::Error for DccParseError {}

impl fmt::Display for DccParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DccParseError::DccType => write!(f, "DccType could not be parsed from string."),
            DccParseError::DccRecord => write!(f, "DccRecord could not be parsed."),
        }
    }
}

impl From<ParseIntError> for DccParseError {
    fn from(_: ParseIntError) -> Self {
        DccParseError::DccRecord
    }
}

impl FromStr for DccType {
    type Err = DccParseError;
    fn from_str(input: &str) -> Result<DccType, DccParseError> {
        match input {
            "SEND" => Ok(DccType::SEND),
            "CHAT" => Ok(DccType::CHAT),
            _ => Err(DccParseError::DccType),
        }
    }
}

impl fmt::Display for DccType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DccType::CHAT => write!(f, "CHAT"),
            DccType::SEND => write!(f, "SEND"),
        }
    }
}

impl DccRecord {
    /// DCC type argument address port [size]
    ///
    /// type     - The connection type.
    /// argument - The connectin type dependant argument.
    /// address  - The host address of the initiator as an integer.
    /// port     - The port or the socket on which the initiator expects
    ///            to receive the connection.
    /// size     - If the connection type is "SEND" (see below), then size
    ///            will indicate the size of the file being offered. Obsolete
    ///            IRCII clients do not send this, so be prepared if this is
    ///            not present.

    /// The following DCC connection types are defined:
    ///
    /// Type    Purpose                                 Argument
    /// CHAT    To carry on a semi-secure conversation  the string "chat"
    /// SEND    To send a file to the recipient         the file name
    pub fn new(origin: &str, receiver: &str, msg: &str) -> Result<DccRecord, DccParseError> {
        // Example msg: SEND "SearchBot_results_for_ python.txt.zip" 2907707975 3078 24999
        let mut param_iter: Vec<&str> = msg.split_whitespace().collect();
        let dcc_type: DccType = param_iter.remove(0).parse()?;
        let file_size = param_iter.pop().and_then(|fs| fs.parse::<u32>().ok());
        let port: u16 = param_iter.pop().ok_or(DccParseError::DccRecord)?.parse()?;

        let address: u32 = param_iter.pop().ok_or(DccParseError::DccRecord)?.parse()?;
        let address_dot_decimal: Ipv4Addr = Ipv4Addr::new(
            (address >> 24) as u8,
            (address >> 16) as u8,
            (address >> 8) as u8,
            (address) as u8,
        );

        let socket_address = SocketAddr::new(IpAddr::V4(address_dot_decimal), port);

        let argument = param_iter.join("");
        let argument = argument.trim_start_matches('"').trim_end_matches('"');

        Ok(DccRecord {
            dcc_type,
            address: socket_address,
            origin: origin.to_string(),
            receiver: receiver.to_string(),
            argument: argument.to_string(),
            file_size,
        })
    }

    pub(crate) fn info(&self) -> DccRecordInfo {
        DccRecordInfo {
            dcc_type: self.dcc_type,
            argument: self.argument.clone(),
            file_size: self.file_size,
        }
    }

    pub(crate) fn argument(&self) -> &String {
        &self.argument
    }

    pub(crate) fn address(&self) -> &SocketAddr {
        &self.address
    }

    pub(crate) fn receiver(&self) -> &String {
        &self.receiver
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_parse() {
        let expected = DccRecord {
            dcc_type: DccType::SEND,
            address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(173, 80, 26, 71)), 3078),
            origin: "Origin".to_string(),
            receiver: "me".to_string(),
            argument: "SearchBot_results_for_python.txt.zip".to_string(),
            file_size: Some(24999),
        };
        let s = r#"SEND "SearchBot_results_for_ python.txt.zip" 2907707975 3078 24999"#;
        let r = DccRecord::new("Origin", "me", s);
        assert_eq!(expected, r.unwrap());

        let s = r#"SEND SearchBot_results_for_python.txt.zip 2907707975 3078 24999"#;
        let r = DccRecord::new("Origin", "me", s);
        assert_eq!(expected, r.unwrap());
    }
}
