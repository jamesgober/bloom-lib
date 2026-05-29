//! The error type shared by every structure in the crate.

use core::fmt;

/// Errors returned by fallible operations across the crate.
///
/// Every constructor that accepts tuning parameters validates them and returns
/// [`Error::InvalidParameter`] rather than panicking. Operations that combine
/// two structures (for example [`BloomFilter::merge`](crate::BloomFilter::merge))
/// return [`Error::IncompatibleParameters`] when the operands were not built
/// with the same dimensions. A Cuckoo filter returns [`Error::CapacityExceeded`]
/// when an insertion cannot find a free slot after exhausting its relocation
/// budget.
///
/// The enum is `#[non_exhaustive]`: matching on it must include a wildcard arm
/// so that future variants do not break downstream code.
///
/// # Examples
///
/// ```
/// use bloom_lib::Error;
///
/// let err = Error::InvalidParameter {
///     param: "rate",
///     reason: "must be in (0.0, 1.0)",
/// };
/// assert_eq!(err.to_string(), "invalid parameter `rate`: must be in (0.0, 1.0)");
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A tuning parameter fell outside its valid range.
    ///
    /// `param` names the offending argument; `reason` states the constraint it
    /// violated. Both are static strings so the variant stays allocation-free
    /// and usable on `no_std` targets.
    InvalidParameter {
        /// The name of the parameter that was rejected.
        param: &'static str,
        /// A human-readable statement of the constraint that was violated.
        reason: &'static str,
    },

    /// Two structures could not be combined because their parameters differ.
    ///
    /// Merging, intersecting, or comparing two probabilistic structures only
    /// has a defined meaning when both were constructed with identical
    /// dimensions (bit count, hash count, register precision, and so on).
    IncompatibleParameters,

    /// An insertion failed because the structure is full.
    ///
    /// Returned by [`CuckooFilter::insert`](crate::CuckooFilter::insert) when no
    /// free slot is found within the relocation budget. It signals that the
    /// filter has reached its practical load limit; create a larger filter.
    CapacityExceeded,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidParameter { param, reason } => {
                write!(f, "invalid parameter `{param}`: {reason}")
            }
            Error::IncompatibleParameters => {
                f.write_str("structures were built with incompatible parameters")
            }
            Error::CapacityExceeded => {
                f.write_str("structure is full: insertion exceeded its capacity")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn test_display_invalid_parameter() {
        let err = Error::InvalidParameter {
            param: "rate",
            reason: "must be in (0.0, 1.0)",
        };
        assert_eq!(
            err.to_string(),
            "invalid parameter `rate`: must be in (0.0, 1.0)"
        );
    }

    #[test]
    fn test_display_incompatible_parameters() {
        assert_eq!(
            Error::IncompatibleParameters.to_string(),
            "structures were built with incompatible parameters"
        );
    }

    #[test]
    fn test_display_capacity_exceeded() {
        assert_eq!(
            Error::CapacityExceeded.to_string(),
            "structure is full: insertion exceeded its capacity"
        );
    }

    #[test]
    fn test_is_std_error() {
        // Confirms the `std::error::Error` impl is wired up.
        fn assert_error<E: std::error::Error>(_: &E) {}
        assert_error(&Error::CapacityExceeded);
    }

    #[test]
    fn test_clone_and_eq() {
        let err = Error::InvalidParameter {
            param: "k",
            reason: "must be greater than zero",
        };
        assert_eq!(err.clone(), err);
        assert_ne!(err, Error::CapacityExceeded);
    }
}
