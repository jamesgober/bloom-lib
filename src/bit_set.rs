//! A compact fixed-size bit set backed by 64-bit words.
//!
//! This is an internal building block for [`BloomFilter`](crate::BloomFilter).
//! It stores `n` bits in `ceil(n / 64)` words and exposes only the operations
//! the filter needs: test, set-and-report, population count, clear, and a
//! word-wise union. Indexing is unchecked in release builds because every
//! caller derives its index from [`crate::hash::reduce`], which is already
//! bounded by the bit count.

use alloc::vec::Vec;

use crate::{util::filled_vec, Error};

/// Number of bits in a backing word.
const WORD_BITS: u64 = 64;

/// A fixed-length sequence of bits packed into `u64` words.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub(crate) struct BitSet {
    words: Vec<u64>,
    num_bits: u64,
}

impl BitSet {
    /// Allocates a bit set with `num_bits` bits, all cleared to zero.
    ///
    /// Fails with [`Error::InvalidParameter`] (naming `num_bits`) when the
    /// word count does not fit in `usize` or cannot be allocated.
    #[inline]
    pub(crate) fn try_new(num_bits: u64) -> Result<Self, Error> {
        let num_words =
            usize::try_from(num_bits.div_ceil(WORD_BITS)).map_err(|_| Error::InvalidParameter {
                param: "num_bits",
                reason: crate::util::TOO_LARGE,
            })?;
        Ok(Self {
            words: filled_vec(0u64, num_words, "num_bits")?,
            num_bits,
        })
    }

    /// The number of bits the set can hold.
    #[inline]
    pub(crate) fn len(&self) -> u64 {
        self.num_bits
    }

    /// Returns `true` if the bit at `index` is set.
    #[inline(always)]
    pub(crate) fn get(&self, index: u64) -> bool {
        let word = (index / WORD_BITS) as usize;
        let bit = index % WORD_BITS;
        (self.words[word] >> bit) & 1 == 1
    }

    /// Sets the bit at `index` and returns its previous value.
    ///
    /// A return of `false` means the bit was newly set by this call.
    #[inline(always)]
    pub(crate) fn set(&mut self, index: u64) -> bool {
        let word = (index / WORD_BITS) as usize;
        let mask = 1u64 << (index % WORD_BITS);
        let previous = self.words[word] & mask != 0;
        self.words[word] |= mask;
        previous
    }

    /// Returns the number of set bits.
    #[inline]
    pub(crate) fn count_ones(&self) -> u64 {
        self.words.iter().map(|w| u64::from(w.count_ones())).sum()
    }

    /// Clears every bit to zero, retaining the allocation.
    #[inline]
    pub(crate) fn clear(&mut self) {
        self.words.iter_mut().for_each(|w| *w = 0);
    }

    /// Returns `true` if the two sets have the same bit length.
    #[inline]
    pub(crate) fn is_compatible(&self, other: &Self) -> bool {
        self.num_bits == other.num_bits
    }

    /// Unions `other` into `self` word by word (bitwise OR).
    ///
    /// The caller must ensure the two sets share a length; see
    /// [`is_compatible`](Self::is_compatible).
    #[inline]
    pub(crate) fn union_with(&mut self, other: &Self) {
        for (dst, src) in self.words.iter_mut().zip(other.words.iter()) {
            *dst |= *src;
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for BitSet {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use crate::util::invalid;

        /// The serialized form, decoded before any invariant is checked.
        #[derive(serde::Deserialize)]
        #[serde(rename = "BitSet")]
        struct Raw {
            words: Vec<u64>,
            num_bits: u64,
        }

        let raw = Raw::deserialize(deserializer)?;
        if raw.num_bits == 0 {
            return Err(invalid("num_bits", "must be greater than zero"));
        }
        if raw.words.len() as u64 != raw.num_bits.div_ceil(WORD_BITS) {
            return Err(invalid(
                "words",
                "word count does not match the bit count (expected ceil(num_bits / 64))",
            ));
        }
        let used = raw.num_bits % WORD_BITS;
        if used != 0 {
            let unused_mask = !0u64 << used;
            if raw.words.last().is_some_and(|last| last & unused_mask != 0) {
                return Err(invalid("words", "bits beyond num_bits are set"));
            }
        }
        Ok(Self {
            words: raw.words,
            num_bits: raw.num_bits,
        })
    }
}
