use std::fmt;

use hex::ToHex;


// This should be in hex crate
pub(crate) struct Hex<T>(pub T);

impl<T: ToHex> fmt::Display for Hex<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.write_hex(f)
    }
}
