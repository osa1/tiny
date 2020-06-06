use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;

#[derive(Debug, PartialEq)]
pub struct DCCRecord {
    /// SEND or CHAT (only supporting SEND right now)
    dcc_type: DCCType,
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
pub enum DCCType {
    SEND,
    CHAT,
}

#[derive(Debug)]
pub struct DCCTypeParseError;

impl std::error::Error for DCCTypeParseError {}

impl fmt::Display for DCCTypeParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DCCType could not be parsed from string.")
    }
}

impl FromStr for DCCType {
    type Err = DCCTypeParseError;
    fn from_str(input: &str) -> Result<DCCType, DCCTypeParseError> {
        match input {
            "SEND" => Ok(DCCType::SEND),
            "CHAT" => Ok(DCCType::CHAT),
            _ => Err(DCCTypeParseError),
        }
    }
}

impl fmt::Display for DCCType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DCCType::CHAT => write!(f, "CHAT"),
            DCCType::SEND => write!(f, "SEND"),
        }
    }
}

impl DCCRecord {
    /// DCC type argument address port [size]
    ///
    /// type	 - The connection type.
    /// argument - The connectin type dependant argument.
    /// address	 - The host address of the initiator as an integer.
    /// port	 - The port or the socket on which the initiator expects
    /// 	       to receive the connection.
    /// size     - If the connection type is "SEND" (see below), then size
    /// 	       will indicate the size of the file being offered. Obsolete
    /// 	       IRCII clients do not send this, so be prepared if this is
    /// 	       not present.

    /// The following DCC connection types are defined:
    ///
    /// Type	Purpose					                Argument
    /// CHAT	To carry on a semi-secure conversation	the string "chat"
    /// SEND	To send a file to the recipient		    the file name
    pub fn from(
        origin: &str,
        receiver: &str,
        msg: &str,
    ) -> Result<DCCRecord, Box<dyn std::error::Error>> {
        let mut param_iter: Vec<&str> = msg.split_whitespace().collect();
        let dcc_type: DCCType = param_iter.remove(0).parse()?;
        let file_size = param_iter.pop().and_then(|fs| fs.parse::<u32>().ok());
        let port: u16 = param_iter.pop().unwrap().parse()?;

        let address: u32 = param_iter.pop().unwrap().parse()?;
        let address_dot_decimal: Ipv4Addr = Ipv4Addr::new(
            (address >> 24) as u8,
            (address >> 16) as u8,
            (address >> 8) as u8,
            (address) as u8,
        );

        let socket_address = SocketAddr::new(IpAddr::V4(address_dot_decimal), port);

        let argument = param_iter.join("");
        let argument = argument.trim_start_matches('"').trim_end_matches('"');

        Ok(DCCRecord {
            dcc_type,
            address: socket_address,
            origin: origin.to_string(),
            receiver: receiver.to_string(),
            argument: argument.to_string(),
            file_size,
        })
    }

    pub fn get_type(&self) -> DCCType {
        self.dcc_type
    }

    pub fn get_addr(&self) -> &SocketAddr {
        &self.address
    }

    pub fn get_argument(&self) -> &String {
        &self.argument
    }

    pub fn get_file_size(&self) -> Option<u32> {
        self.file_size
    }

    pub fn get_receiver(&self) -> &String {
        &self.receiver
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_parse() {
        let expected = DCCRecord {
            dcc_type: DCCType::SEND,
            address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(173, 80, 26, 71)), 3078),
            origin: "Origin".to_string(),
            receiver: "me".to_string(),
            argument: "SearchBot_results_for_python.txt.zip".to_string(),
            file_size: Some(24999),
        };
        let s = r#"SEND "SearchBot_results_for_ python.txt.zip" 2907707975 3078 24999"#;
        let r = DCCRecord::from("Origin", "me", s);
        assert_eq!(expected, r.unwrap());

        let s = r#"SEND SearchBot_results_for_python.txt.zip 2907707975 3078 24999"#;
        let r = DCCRecord::from("Origin", "me", s);
        assert_eq!(expected, r.unwrap());
    }
}
