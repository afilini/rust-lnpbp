// LNP/BP Rust Library
// Written in 2020 by
//     Dr. Maxim Orlovsky <orlovsky@pandoracore.com>
//
// To the extent possible under law, the author(s) have dedicated all
// copyright and related and neighboring rights to this software to
// the public domain worldwide. This software is distributed without
// any warranty.
//
// You should have received a copy of the MIT License
// along with this software.
// If not, see <https://opensource.org/licenses/MIT>.

//! BOLT-8 related structures and functions covering Lightning network
//! transport layer

use std::io;
use std::fmt;
use std::str::FromStr;
use std::net::SocketAddr;
use std::convert::TryInto;

#[cfg(feature="use-tokio")]
use tokio::net::TcpStream;
#[cfg(feature="use-tokio")]
use tokio::io::AsyncWriteExt;
#[cfg(feature="use-tokio")]
use tokio::io::AsyncReadExt;

#[cfg(not(feature="use-tokio"))]
use std::net::TcpStream;
#[cfg(not(feature="use-tokio"))]
use std::io::AsyncWriteExt;
#[cfg(not(feature="use-tokio"))]
use std::io::AsyncReadExt;

use lightning::secp256k1;

// We re-export this under more proper name (it's not per-channel encryptor,
// it is per-connection transport-level encryptor)
use lightning::ln::peers::conduit::Conduit as Encryptor;
use lightning::ln::peers::handshake::PeerHandshake;

use crate::common::internet::InetSocketAddr;
use super::LIGHTNING_P2P_DEFAULT_PORT;


pub const MAX_TRANSPORT_FRAME_SIZE: usize = 65569;

#[derive(Clone, Copy, Debug)]
pub struct NodeAddr {
    pub node_id: secp256k1::PublicKey,
    pub inet_addr: InetSocketAddr,
}

impl NodeAddr {
    pub async fn connect(&self,
                   private_key: &secp256k1::SecretKey,
                   ephemeral_private_key: &secp256k1::SecretKey
    ) -> Result<Connection, ConnectionError> {
        Connection::new(self, private_key, ephemeral_private_key).await
    }
}

impl fmt::Display for NodeAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.node_id, self.inet_addr)
    }
}

impl FromStr for NodeAddr {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err_msg = "Wrong LN peer id; it must be in format \
                            `<node_id>@<node_inet_addr>[:<port>]`, \
                            where <node_inet_addr> may be IPv4, IPv6 or TORv3 address\
                            ";

        let mut splitter = s.split('@');
        let (id, inet) = match (splitter.next(), splitter.next(), splitter.next()) {
            (Some(id), Some(inet), None) => (id, inet),
            _ => Err(String::from(err_msg))?
        };

        let mut splitter = inet.split(':');
        let (addr, port) = match (splitter.next(), splitter.next(), splitter.next()) {
            (Some(addr), Some(port), None) =>
                (addr, port.parse().map_err(|_| err_msg)?),
            (Some(addr), None, _) => (addr, LIGHTNING_P2P_DEFAULT_PORT),
            _ => Err(String::from(err_msg))?
        };

        Ok(Self {
            node_id: id.parse().map_err(|_| err_msg)?,
            inet_addr: InetSocketAddr::new(addr.parse().map_err(|_| err_msg)?, port)
        })
    }
}


#[derive(Debug, Display)]
#[display_from(Debug)]
pub enum ConnectionError {
    TorNotYetSupported,
    FailedHandshake(String),
    IoError(io::Error)
}

impl From<io::Error> for ConnectionError {
    fn from(err: io::Error) -> Self {
        ConnectionError::IoError(err)
    }
}


pub struct Connection {
    pub stream: TcpStream,
    pub outbound: bool,
    encryptor: Encryptor,
}

impl Connection {
    pub async fn new(node: &NodeAddr,
                     private_key: &secp256k1::SecretKey,
                     ephemeral_private_key: &secp256k1::SecretKey
    ) -> Result<Self, ConnectionError> {

        // TODO: Add support for Tor connections
        if node.inet_addr.address.is_tor() {
            Err(ConnectionError::TorNotYetSupported)?
        }

        #[cfg(feature="use-log")]
        debug!("Initiating connection protocol with {}", node);

        // Opening network connection
        #[cfg(feature="use-tor")]
        let socket_addr: SocketAddr = node.inet_addr.try_into().unwrap();
        #[cfg(not(feature="use-tor"))]
        let socket_addr: SocketAddr = node.inet_addr.into();

        #[cfg(feature="use-log")]
        trace!("Connecting to {}", socket_addr);
        let mut stream = TcpStream::connect(socket_addr).await?;

        #[cfg(feature="use-log")]
        trace!("Starting handshake procedure with {}", node);
        let mut handshake = PeerHandshake::new_outbound(
            private_key, &node.node_id, ephemeral_private_key
        );

        let mut step = 0;
        let mut input: &[u8] = &[];
        let mut buf = vec![];
        buf.reserve(MAX_TRANSPORT_FRAME_SIZE);
        let result: Result<Encryptor, ConnectionError> = loop {
            #[cfg(feature="use-log")]
            trace!("Handshake step {}: processing data `{:x?}`", step, input);

            let (act, enc) = handshake.process_act(input)
                .map_err(|msg| ConnectionError::FailedHandshake(msg))?;

            if let Some(encryptor) = enc {
                break Ok(encryptor)
            } else if let Some(act) = act {
                #[cfg(feature="use-log")]
                trace!("Handshake step {}: sending `{:x?}`", step, act.serialize());

                stream.write_all(&act.serialize()).await?;
            } else {
                #[cfg(feature="use-log")]
                error!("`PeerHandshake.process_act` returned non-standard result");

                Err(ConnectionError::FailedHandshake(
                    "PeerHandshake.process_act returned non-standard result"
                        .to_string()
                ))?
            }

            #[cfg(feature="use-log")]
            trace!("Handshake step {}: waiting for response`", step);

            let read_len = stream.read_buf(&mut buf).await?;
            input = &buf[0..read_len];

            #[cfg(feature="use-log")]
            trace!("Handshake step {}: received data `{:x?}`", step, input);

            step += 1;
        };
        let encryptor = result?;

        #[cfg(feature="use-log")]
        trace!("Handshake successfully completed");

        Ok(Self {
            stream,
            outbound: true,
            encryptor
        })
    }
}