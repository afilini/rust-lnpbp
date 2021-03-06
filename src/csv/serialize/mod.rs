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

#[macro_use]
pub mod commitment;
#[macro_use]
pub mod network;
pub mod storage;

pub use commitment::*;
pub use network::*;
pub use storage::*;


use std::{str, convert::From};

#[derive(Debug, Display)]
#[display_from(Debug)]
#[non_exhaustive]
pub enum Error {
    BitcoinConsensus(bitcoin::consensus::encode::Error),
    EnumValueUnknown(u8),
    EnumValueOverflow,
    Utf8Error(str::Utf8Error),
    ValueOutOfRange,
    WrongOptionalEncoding,
    WrongDataSize { expected: usize, found: usize },
    ParseFailed(&'static str),
}

impl From<str::Utf8Error> for Error {
    fn from(err: str::Utf8Error) -> Self {
        Self::Utf8Error(err)
    }
}

impl From<bitcoin::consensus::encode::Error> for Error {
    #[inline]
    fn from(err: bitcoin::consensus::encode::Error) -> Self {
        Error::BitcoinConsensus(err)
    }
}
