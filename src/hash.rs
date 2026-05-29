//! Hashing infrastructure shared by every structure.
//!
//! Probabilistic data structures live and die by the quality of their hash
//! function: poor mixing inflates the false-positive rate and skews cardinality
//! estimates. This module supplies a fast, high-quality, **deterministic**
//! default hasher and the machinery to derive the multiple index hashes each
//! structure needs from a single pass over the input.
//!
//! # Choosing a hasher
//!
//! Every structure is generic over [`core::hash::BuildHasher`] and defaults to
//! [`DefaultHashBuilder`]. The default is seeded with a fixed constant, which
//! makes hashing reproducible: two filters built with the default hasher are
//! always mergeable, and serialized state round-trips byte-for-byte across
//! runs. The trade-off is that a fixed seed offers no defence against an
//! adversary who crafts inputs to collide on purpose. When the inputs are
//! untrusted, substitute a randomly-seeded [`BuildHasher`] such as
//! [`std::collections::hash_map::RandomState`]:
//!
//! ```
//! # #[cfg(feature = "std")] {
//! use std::collections::hash_map::RandomState;
//! use bloom_lib::BloomFilter;
//!
//! let filter: BloomFilter<&str, RandomState> =
//!     BloomFilter::with_hasher(1_000, 0.01, RandomState::new()).unwrap();
//! # }
//! ```

use core::hash::{BuildHasher, Hasher};

/// Fixed seed for [`DefaultHasher`]. Chosen for a good bit distribution; the
/// value itself is arbitrary but must stay constant for reproducibility.
const SEED: u64 = 0x2545_F491_4F6C_DD1D;

/// Odd multiplier used by the block mixer. Large, odd, and high-entropy.
const MIX_A: u64 = 0xFF51_AFD7_ED55_8CCD;

/// Second multiplier, applied in the finalizer to decorrelate the result.
const MIX_B: u64 = 0xC4CE_B9FE_1A85_EC53;

/// 64x64 -> 64 multiply-fold, the core mixing step (the "wymum" primitive).
///
/// Multiplying two 64-bit words yields a 128-bit product whose high and low
/// halves are then folded together with XOR. This propagates every input bit
/// across the whole output and is the engine behind several modern hashes.
#[inline(always)]
fn fold_mul(a: u64, b: u64) -> u64 {
    let product = (a as u128).wrapping_mul(b as u128);
    (product as u64) ^ ((product >> 64) as u64)
}

/// A fast, non-cryptographic 64-bit hasher with strong avalanche behaviour.
///
/// `DefaultHasher` folds the input eight bytes at a time through a 64x64-bit
/// multiply-fold and mixes the total length into the finalizer, so inputs that
/// differ only by trailing zero bytes do not collide. It is **not** suitable for
/// authentication or any setting where collision resistance against an
/// adversary is required; use a cryptographic hash for that.
///
/// You rarely construct this directly. It is produced by
/// [`DefaultHashBuilder`], which the structures use by default.
///
/// # Examples
///
/// ```
/// use core::hash::{Hash, Hasher};
/// use bloom_lib::hash::DefaultHasher;
///
/// let mut hasher = DefaultHasher::default();
/// "probabilistic".hash(&mut hasher);
/// let digest = hasher.finish();
///
/// // Hashing the same value again yields the same digest.
/// let mut again = DefaultHasher::default();
/// "probabilistic".hash(&mut again);
/// assert_eq!(digest, again.finish());
/// ```
#[derive(Debug, Clone)]
pub struct DefaultHasher {
    state: u64,
    len: u64,
}

impl DefaultHasher {
    /// Creates a hasher seeded with the given value.
    ///
    /// Two hashers created with the same seed produce identical digests for
    /// identical inputs. [`DefaultHasher::default`] uses a fixed library seed.
    ///
    /// # Examples
    ///
    /// ```
    /// use core::hash::Hasher;
    /// use bloom_lib::hash::DefaultHasher;
    ///
    /// let mut a = DefaultHasher::with_seed(42);
    /// let mut b = DefaultHasher::with_seed(7);
    /// a.write(b"payload");
    /// b.write(b"payload");
    /// assert_ne!(a.finish(), b.finish());
    /// ```
    #[inline]
    #[must_use]
    pub const fn with_seed(seed: u64) -> Self {
        Self {
            state: seed,
            len: 0,
        }
    }
}

impl Default for DefaultHasher {
    #[inline]
    fn default() -> Self {
        Self::with_seed(SEED)
    }
}

impl Hasher for DefaultHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        self.len = self.len.wrapping_add(bytes.len() as u64);

        let mut rest = bytes;
        while rest.len() >= 8 {
            let mut block = [0u8; 8];
            block.copy_from_slice(&rest[..8]);
            self.state = fold_mul(self.state ^ u64::from_le_bytes(block), MIX_A);
            rest = &rest[8..];
        }

        if !rest.is_empty() {
            let mut block = [0u8; 8];
            block[..rest.len()].copy_from_slice(rest);
            self.state = fold_mul(self.state ^ u64::from_le_bytes(block), MIX_A);
        }
    }

    #[inline]
    fn finish(&self) -> u64 {
        fold_mul(self.state ^ self.len, MIX_B)
    }
}

/// A [`BuildHasher`] that yields [`DefaultHasher`] instances seeded with a fixed
/// constant.
///
/// This is the default hasher for every structure in the crate. Because the
/// seed is fixed, the resulting hashing is deterministic: ideal for
/// reproducible builds, mergeable filters, and stable serialized state. It is a
/// zero-sized type, so embedding it in a structure costs nothing.
///
/// # Examples
///
/// ```
/// use core::hash::BuildHasher;
/// use bloom_lib::hash::DefaultHashBuilder;
///
/// let builder = DefaultHashBuilder::default();
/// let h1 = builder.hash_one("key");
/// let h2 = builder.hash_one("key");
/// assert_eq!(h1, h2);
/// ```
#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DefaultHashBuilder;

impl BuildHasher for DefaultHashBuilder {
    type Hasher = DefaultHasher;

    #[inline]
    fn build_hasher(&self) -> Self::Hasher {
        DefaultHasher::default()
    }
}

/// A pair of independent 64-bit hashes used to synthesise an arbitrary number
/// of index hashes via the Kirsch-Mitzenmacher double-hashing scheme.
///
/// The structures need `k` distinct hash positions per item but hash the item
/// only once. From the base digest `h1` and a decorrelated companion `h2`, the
/// `i`-th position is `h1 + i * h2`. This yields `k` well-distributed hashes at
/// the cost of a single hashing pass.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone, Copy)]
pub(crate) struct HashPair {
    h1: u64,
    h2: u64,
}

#[cfg(feature = "alloc")]
impl HashPair {
    /// Hashes `item` once with `build_hasher` and derives the companion hash.
    #[inline]
    pub(crate) fn new<T, S>(item: &T, build_hasher: &S) -> Self
    where
        T: core::hash::Hash + ?Sized,
        S: BuildHasher,
    {
        let h1 = build_hasher.hash_one(item);
        // Derive a second, decorrelated hash from the first with a strong mix.
        // Forcing it odd guarantees the `h1 + i * h2` sequence visits a long
        // cycle of residues regardless of the modulus.
        let h2 = fold_mul(h1, MIX_B) | 1;
        Self { h1, h2 }
    }

    /// The `i`-th synthesised 64-bit hash.
    #[inline(always)]
    pub(crate) fn nth(&self, i: u64) -> u64 {
        self.h1.wrapping_add(i.wrapping_mul(self.h2))
    }
}

/// Maps a 64-bit hash uniformly into `[0, range)` without a division.
///
/// This is Lemire's multiply-shift reduction: it takes the high 64 bits of
/// `hash * range`, which is both faster than `hash % range` and free of the
/// modulo bias that `%` introduces when `range` is not a power of two.
#[cfg(feature = "alloc")]
#[inline(always)]
pub(crate) fn reduce(hash: u64, range: u64) -> u64 {
    (((hash as u128).wrapping_mul(range as u128)) >> 64) as u64
}

/// A strong 64-bit avalanche mix, used to derive a secondary hash from a small
/// value (such as a Cuckoo fingerprint) where the input has little entropy.
#[cfg(feature = "alloc")]
#[inline(always)]
pub(crate) fn mix64(x: u64) -> u64 {
    fold_mul(x ^ MIX_B, MIX_A)
}
