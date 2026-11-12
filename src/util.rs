//! Small internal helpers shared by the structures.

use alloc::vec::Vec;

use crate::Error;

/// Reason reported when a structure's size cannot be allocated.
pub(crate) const TOO_LARGE: &str = "is too large: the allocation failed or overflowed";

/// Allocates a vector holding `len` copies of `value`.
///
/// Unlike `vec![value; len]`, a failed allocation returns
/// [`Error::InvalidParameter`] naming `param` instead of aborting the process,
/// so a size taken from configuration or from a peer cannot crash the caller.
pub(crate) fn filled_vec<T: Clone>(
    value: T,
    len: usize,
    param: &'static str,
) -> Result<Vec<T>, Error> {
    let mut vec = Vec::new();
    vec.try_reserve_exact(len)
        .map_err(|_| Error::InvalidParameter {
            param,
            reason: TOO_LARGE,
        })?;
    vec.resize(len, value);
    Ok(vec)
}

/// Builds a deserialization error from a parameter name and a reason, using
/// the same wording as [`Error::InvalidParameter`].
#[cfg(feature = "serde")]
pub(crate) fn invalid<E: serde::de::Error>(param: &'static str, reason: &'static str) -> E {
    E::custom(Error::InvalidParameter { param, reason })
}
