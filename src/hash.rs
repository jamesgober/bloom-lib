//! Hashing infrastructure shared by every structure.
//!
//! Probabilistic data structures live and die by the quality of their hash
//! function: poor mixing inflates the false-positive rate and skews cardinality
//! estimates. This module supplies a fast, **deterministic** default hasher, a
//! keyed hasher for untrusted input, and the machinery to derive the multiple
//! index hashes each structure needs from a single pass over the input.
//!
//! # Choosing a hasher
//!
//! Every structure is generic over [`core::hash::BuildHasher`] and defaults to
//! [`DefaultHashBuilder`]. The default is seeded with a fixed constant, which
//! makes hashing reproducible: two filters built with the default hasher are
//! always mergeable, and serialized state can be saved and queried later, on
//! the same machine or another one.
//!
//! The fixed seed offers no defence against an adversary who picks inputs on
//! purpose. Collisions in [`DefaultHasher`] can be computed directly, without
//! any search, so an attacker who controls the keys can force false positives,
//! inflate counts, hide distinct items from a cardinality estimate, or fill a
//! Cuckoo filter with a handful of keys. When the inputs are untrusted, use
//! [`KeyedHashBuilder`] with a secret key:
//!
//! ```
//! # #[cfg(feature = "std")] {
//! use bloom_lib::{hash::KeyedHashBuilder, BloomFilter};
//!
//! let hasher = KeyedHashBuilder::random();
//! let mut filter: BloomFilter<str, KeyedHashBuilder> =
//!     BloomFilter::with_hasher(1_000, 0.01, hasher).unwrap();
//! filter.insert("attacker-chosen");
//! assert!(filter.contains("attacker-chosen"));
//! # }
//! ```
//!
//! # Hashers and serialized state
//!
//! Serialized state never contains the hasher. Deserializing rebuilds it with
//! `S::default()`, which is correct for [`DefaultHashBuilder`] and for any
//! other builder whose `Default` always produces the same hasher. It is wrong
//! for a randomly seeded builder such as
//! `std::collections::hash_map::RandomState`: its `Default` draws a new key,
//! so a restored filter hashes every item differently and reports almost all
//! of them absent. Do not persist structures built with `RandomState`.
//!
//! [`KeyedHashBuilder`] does not implement `Default`, so a structure using it
//! cannot be deserialized by accident with the wrong key. Save the key with
//! [`KeyedHashBuilder::key`], deserialize the structure with the default
//! hasher type (the serialized form is the same), and attach the key again
//! with `replace_hasher`:
//!
//! ```
//! # #[cfg(all(feature = "std", feature = "serde"))] {
//! use bloom_lib::{hash::KeyedHashBuilder, BloomFilter};
//!
//! let key = [7u8; 16]; // load this from your secret store
//! let mut filter: BloomFilter<str, KeyedHashBuilder> =
//!     BloomFilter::with_hasher(1_000, 0.01, KeyedHashBuilder::new(key)).unwrap();
//! filter.insert("stored");
//! let json = serde_json::to_string(&filter).unwrap();
//!
//! let restored: BloomFilter<str> = serde_json::from_str(&json).unwrap();
//! let restored = restored.replace_hasher(KeyedHashBuilder::new(key));
//! assert!(restored.contains("stored"));
//! # }
//! ```
//!
//! # Portability
//!
//! Both hashers feed integers to the hash in little-endian byte order and
//! widen `usize` and `isize` (including the length prefix that `[T]`, `str`
//! and `Vec<T>` write) to 64 bits, so a given key hashes to the same value on
//! every target, 32-bit or 64-bit, little-endian or big-endian. The values
//! match what bloom-lib 1.0.0 produced on x86_64 and aarch64, so state
//! serialized there stays valid.
//!
//! Two limits come from the standard library rather than from this crate:
//!
//! - A slice, array or `Vec` of multi-byte integers (`[u32]`, `Vec<u64>`, ...)
//!   is passed to the hasher as one block of native-endian bytes, so it hashes
//!   differently on big-endian targets. Byte slices (`[u8]`), `str`, single
//!   integers and structs or tuples of them are not affected.
//! - The standard library does not promise that the bytes a `Hash` impl feeds
//!   to the hasher stay the same between Rust releases. They have not changed
//!   for `[u8]`, `str` or the integer types, and this crate's tests pin the
//!   hash values, but for long-lived persisted state prefer `[u8]` or `str`
//!   keys.

use core::hash::{BuildHasher, Hasher};

/// Fixed seed for [`DefaultHasher`]. Chosen for a good bit distribution; the
/// value itself is arbitrary but must stay constant for reproducibility.
const SEED: u64 = 0x2545_f491_4f6c_dd1d;

/// Odd multiplier used by the block mixer. Large, odd, and high-entropy.
const MIX_A: u64 = 0xff51_afd7_ed55_8ccd;

/// Second multiplier, applied in the finalizer to decorrelate the result.
const MIX_B: u64 = 0xc4ce_b9fe_1a85_ec53;

/// Overrides every integer `write_*` method of [`Hasher`] so integers are fed
/// as little-endian bytes and `usize`/`isize` are widened to 64 bits.
///
/// The provided `Hasher` methods use native-endian bytes and the native
/// pointer width, which made the same key hash differently on 32-bit and
/// big-endian targets. On 64-bit little-endian targets these overrides feed
/// exactly the bytes the provided methods fed, so hash values there are
/// unchanged.
macro_rules! portable_integer_writes {
    () => {
        #[inline]
        fn write_u8(&mut self, i: u8) {
            self.write(&[i]);
        }

        #[inline]
        fn write_u16(&mut self, i: u16) {
            self.write(&i.to_le_bytes());
        }

        #[inline]
        fn write_u32(&mut self, i: u32) {
            self.write(&i.to_le_bytes());
        }

        #[inline]
        fn write_u64(&mut self, i: u64) {
            self.write(&i.to_le_bytes());
        }

        #[inline]
        fn write_u128(&mut self, i: u128) {
            self.write(&i.to_le_bytes());
        }

        #[inline]
        fn write_usize(&mut self, i: usize) {
            // `usize` is at most 64 bits on every supported target.
            self.write(&(i as u64).to_le_bytes());
        }

        #[inline]
        fn write_i8(&mut self, i: i8) {
            self.write_u8(i as u8);
        }

        #[inline]
        fn write_i16(&mut self, i: i16) {
            self.write_u16(i as u16);
        }

        #[inline]
        fn write_i32(&mut self, i: i32) {
            self.write_u32(i as u32);
        }

        #[inline]
        fn write_i64(&mut self, i: i64) {
            self.write_u64(i as u64);
        }

        #[inline]
        fn write_i128(&mut self, i: i128) {
            self.write_u128(i as u128);
        }

        #[inline]
        fn write_isize(&mut self, i: isize) {
            // Sign-extend to 64 bits so 32-bit targets match 64-bit ones.
            self.write_u64(i as i64 as u64);
        }
    };
}

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

/// A fast, non-cryptographic 64-bit hasher with good statistical mixing.
///
/// `DefaultHasher` folds the input eight bytes at a time through a 64x64-bit
/// multiply-fold and mixes the total length into the finalizer, so inputs that
/// differ only by trailing zero bytes do not collide. Integers are fed in
/// little-endian order and `usize` is widened to 64 bits, so digests are the
/// same on every target.
///
/// It is **not** collision resistant: anyone who knows the seed (and the
/// default seed is public) can construct colliding inputs directly. For keys
/// an adversary can choose, use [`KeyedHasher`] through [`KeyedHashBuilder`].
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

    portable_integer_writes!();
}

/// A [`BuildHasher`] that yields [`DefaultHasher`] instances seeded with a fixed
/// constant.
///
/// This is the default hasher for every structure in the crate. Because the
/// seed is fixed, the resulting hashing is deterministic: ideal for
/// reproducible builds, mergeable filters, and stable serialized state. It is a
/// zero-sized type, so embedding it in a structure costs nothing.
///
/// The fixed seed means collisions can be computed by anyone. Use
/// [`KeyedHashBuilder`] when an adversary can choose the inputs.
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

/// SipHash initialisation constants ("somepseudorandomlygeneratedbytes").
const SIP_INIT: [u64; 4] = [
    0x736f_6d65_7073_6575,
    0x646f_7261_6e64_6f6d,
    0x6c79_6765_6e65_7261,
    0x7465_6462_7974_6573,
];

/// A keyed 64-bit hasher: SipHash-2-4 with a 128-bit secret key.
///
/// SipHash is a pseudorandom function: without the key, an attacker cannot
/// compute hash values or construct collisions, which is what protects a
/// filter or sketch fed with attacker-chosen keys. It is slower than
/// [`DefaultHasher`] (roughly two to three times for short keys), so use it
/// where inputs are untrusted.
///
/// Integers are fed in little-endian order and `usize` is widened to 64 bits,
/// so for a given key the digests are the same on every target.
///
/// You rarely construct this directly. It is produced by
/// [`KeyedHashBuilder`].
///
/// # Examples
///
/// ```
/// use core::hash::{Hash, Hasher};
/// use bloom_lib::hash::KeyedHasher;
///
/// let key = [0x42u8; 16];
/// let mut a = KeyedHasher::new(key);
/// let mut b = KeyedHasher::new(key);
/// "payload".hash(&mut a);
/// "payload".hash(&mut b);
/// assert_eq!(a.finish(), b.finish());
/// ```
#[derive(Clone)]
pub struct KeyedHasher {
    v: [u64; 4],
    /// Pending input bytes that do not yet fill a 64-bit word, little-endian.
    tail: u64,
    /// Number of valid bytes in `tail` (`0..8`).
    ntail: usize,
    /// Total number of bytes written, modulo 2^64.
    length: u64,
}

impl KeyedHasher {
    /// Creates a hasher with the given 128-bit key.
    ///
    /// The key is read as two little-endian 64-bit words, as in the SipHash
    /// reference implementation.
    #[inline]
    #[must_use]
    pub const fn new(key: [u8; 16]) -> Self {
        let (k0, k1) = split_key(key);
        Self {
            v: [
                k0 ^ SIP_INIT[0],
                k1 ^ SIP_INIT[1],
                k0 ^ SIP_INIT[2],
                k1 ^ SIP_INIT[3],
            ],
            tail: 0,
            ntail: 0,
            length: 0,
        }
    }

    #[inline(always)]
    fn compress(&mut self, m: u64) {
        self.v[3] ^= m;
        sip_round(&mut self.v);
        sip_round(&mut self.v);
        self.v[0] ^= m;
    }
}

impl core::fmt::Debug for KeyedHasher {
    // The state is derived from the secret key, so it is not printed.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("KeyedHasher").finish_non_exhaustive()
    }
}

/// Splits a 16-byte key into two little-endian 64-bit words.
#[inline]
const fn split_key(key: [u8; 16]) -> (u64, u64) {
    let k0 = u64::from_le_bytes([
        key[0], key[1], key[2], key[3], key[4], key[5], key[6], key[7],
    ]);
    let k1 = u64::from_le_bytes([
        key[8], key[9], key[10], key[11], key[12], key[13], key[14], key[15],
    ]);
    (k0, k1)
}

/// Loads up to eight bytes as a little-endian word, zero-padded.
#[inline(always)]
fn load_le(bytes: &[u8]) -> u64 {
    let mut block = [0u8; 8];
    block[..bytes.len()].copy_from_slice(bytes);
    u64::from_le_bytes(block)
}

/// One SipRound.
#[inline(always)]
fn sip_round(v: &mut [u64; 4]) {
    v[0] = v[0].wrapping_add(v[1]);
    v[1] = v[1].rotate_left(13);
    v[1] ^= v[0];
    v[0] = v[0].rotate_left(32);
    v[2] = v[2].wrapping_add(v[3]);
    v[3] = v[3].rotate_left(16);
    v[3] ^= v[2];
    v[0] = v[0].wrapping_add(v[3]);
    v[3] = v[3].rotate_left(21);
    v[3] ^= v[0];
    v[2] = v[2].wrapping_add(v[1]);
    v[1] = v[1].rotate_left(17);
    v[1] ^= v[2];
    v[2] = v[2].rotate_left(32);
}

impl Hasher for KeyedHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        self.length = self.length.wrapping_add(bytes.len() as u64);
        let mut rest = bytes;

        if self.ntail != 0 {
            let needed = 8 - self.ntail;
            let take = needed.min(rest.len());
            self.tail |= load_le(&rest[..take]) << (8 * self.ntail);
            if take < needed {
                self.ntail += take;
                return;
            }
            let m = self.tail;
            self.compress(m);
            self.tail = 0;
            self.ntail = 0;
            rest = &rest[take..];
        }

        while rest.len() >= 8 {
            self.compress(load_le(&rest[..8]));
            rest = &rest[8..];
        }

        self.tail = load_le(rest);
        self.ntail = rest.len();
    }

    #[inline]
    fn finish(&self) -> u64 {
        let b = ((self.length & 0xff) << 56) | self.tail;
        let mut v = self.v;
        v[3] ^= b;
        sip_round(&mut v);
        sip_round(&mut v);
        v[0] ^= b;
        v[2] ^= 0xff;
        sip_round(&mut v);
        sip_round(&mut v);
        sip_round(&mut v);
        sip_round(&mut v);
        v[0] ^ v[1] ^ v[2] ^ v[3]
    }

    portable_integer_writes!();
}

/// A [`BuildHasher`] that yields [`KeyedHasher`] instances with a secret key.
///
/// Use this instead of [`DefaultHashBuilder`] when an adversary can choose
/// the items: with a secret key, collisions cannot be constructed, so the
/// structures keep their documented error rates under attack.
///
/// Two structures can only be merged, and a deserialized structure only
/// answers correctly, when they use the same key. The key is not part of the
/// serialized state of a structure; store it separately (for example in your
/// secret store) and reattach it with the structure's `replace_hasher` method
/// after deserializing. The builder itself implements `Serialize` and
/// `Deserialize` (with the `serde` feature) for that purpose; treat the
/// result as a secret.
///
/// `KeyedHashBuilder` deliberately does not implement `Default`, so a
/// structure using it cannot be deserialized with a made-up key.
///
/// # Examples
///
/// ```
/// use bloom_lib::{hash::KeyedHashBuilder, CuckooFilter};
///
/// let hasher = KeyedHashBuilder::new([0x5au8; 16]);
/// let mut filter: CuckooFilter<[u8], KeyedHashBuilder> =
///     CuckooFilter::with_hasher(1_000, hasher).unwrap();
/// filter.insert(b"client-supplied").unwrap();
/// assert!(filter.contains(b"client-supplied"));
/// ```
#[derive(Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeyedHashBuilder {
    key: [u8; 16],
}

impl KeyedHashBuilder {
    /// Creates a builder with the given 128-bit secret key.
    ///
    /// Use a key from a cryptographically secure random source, and keep it
    /// secret: anyone who knows it can construct collisions.
    #[inline]
    #[must_use]
    pub const fn new(key: [u8; 16]) -> Self {
        Self { key }
    }

    /// Creates a builder with a fresh random key, drawn from the operating
    /// system's random source through the standard library's `RandomState`.
    ///
    /// Save the key with [`key`](Self::key) if the structure will be
    /// persisted.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::hash::KeyedHashBuilder;
    ///
    /// let a = KeyedHashBuilder::random();
    /// let b = KeyedHashBuilder::random();
    /// assert_ne!(a.key(), b.key());
    /// ```
    #[cfg(feature = "std")]
    #[must_use]
    pub fn random() -> Self {
        use std::collections::hash_map::RandomState;
        // Each `RandomState` is keyed from the OS random source (and then
        // varied per instance), so the two outputs are unpredictable.
        let k0 = RandomState::new().build_hasher().finish();
        let k1 = RandomState::new().build_hasher().finish();
        let mut key = [0u8; 16];
        key[..8].copy_from_slice(&k0.to_le_bytes());
        key[8..].copy_from_slice(&k1.to_le_bytes());
        Self { key }
    }

    /// Returns the secret key, so it can be stored and the builder rebuilt
    /// with [`new`](Self::new).
    #[inline]
    #[must_use]
    pub const fn key(&self) -> [u8; 16] {
        self.key
    }
}

impl core::fmt::Debug for KeyedHashBuilder {
    // Never print the secret key.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("KeyedHashBuilder").finish_non_exhaustive()
    }
}

impl BuildHasher for KeyedHashBuilder {
    type Hasher = KeyedHasher;

    #[inline]
    fn build_hasher(&self) -> Self::Hasher {
        KeyedHasher::new(self.key)
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

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use core::hash::Hash;

    /// The SipHash reference key `00 01 .. 0f`.
    fn reference_key() -> [u8; 16] {
        let mut key = [0u8; 16];
        for (i, byte) in key.iter_mut().enumerate() {
            *byte = i as u8;
        }
        key
    }

    #[test]
    fn test_keyed_hasher_matches_siphash_reference_vector() {
        // First vector of the SipHash-2-4 paper: empty message.
        let hasher = KeyedHasher::new(reference_key());
        assert_eq!(hasher.finish(), 0x726f_db47_dd0e_0e31);
    }

    #[test]
    #[allow(deprecated)]
    fn test_keyed_hasher_matches_core_siphash_for_every_length() {
        // `core::hash::SipHasher` is documented as SipHash-2-4. It is
        // deprecated for general use but is a fixed, specified algorithm.
        let key = reference_key();
        let (k0, k1) = split_key(key);
        let message: Vec<u8> = (0u8..=200).collect();
        for len in 0..message.len() {
            let mut ours = KeyedHasher::new(key);
            ours.write(&message[..len]);
            let mut theirs = core::hash::SipHasher::new_with_keys(k0, k1);
            theirs.write(&message[..len]);
            assert_eq!(ours.finish(), theirs.finish(), "length {len}");
        }
    }

    #[test]
    fn test_keyed_hasher_streaming_matches_one_shot() {
        let key = reference_key();
        let message: Vec<u8> = (0u8..64).collect();
        let mut one_shot = KeyedHasher::new(key);
        one_shot.write(&message);
        for split_a in 0..message.len() {
            for split_b in split_a..message.len() {
                let mut parts = KeyedHasher::new(key);
                parts.write(&message[..split_a]);
                parts.write(&message[split_a..split_b]);
                parts.write(&message[split_b..]);
                assert_eq!(parts.finish(), one_shot.finish());
            }
        }
    }

    #[test]
    fn test_keyed_hasher_depends_on_key() {
        let a = KeyedHashBuilder::new([1u8; 16]).hash_one("item");
        let b = KeyedHashBuilder::new([2u8; 16]).hash_one("item");
        assert_ne!(a, b);
    }

    #[test]
    fn test_keyed_builder_debug_hides_key() {
        let builder = KeyedHashBuilder::new([0xab; 16]);
        let text = format!("{builder:?} {:?}", builder.build_hasher());
        assert!(!text.contains("171"), "{text}");
        assert!(!text.to_lowercase().contains("ab"), "{text}");
    }

    #[test]
    fn test_integer_writes_are_little_endian_and_64_bit() {
        fn digest(f: impl FnOnce(&mut DefaultHasher)) -> u64 {
            let mut h = DefaultHasher::default();
            f(&mut h);
            h.finish()
        }
        let bytes = digest(|h| h.write(&0x0102_0304u32.to_le_bytes()));
        assert_eq!(digest(|h| h.write_u32(0x0102_0304)), bytes);
        assert_eq!(digest(|h| h.write_i32(0x0102_0304)), bytes);

        let wide = digest(|h| h.write(&7u64.to_le_bytes()));
        assert_eq!(digest(|h| h.write_usize(7)), wide);
        assert_eq!(digest(|h| h.write_u64(7)), wide);

        let neg = digest(|h| h.write(&(-7i64).to_le_bytes()));
        assert_eq!(digest(|h| h.write_isize(-7)), neg);

        // A slice hashes its length as a 64-bit little-endian prefix.
        let slice = digest(|h| [9u8, 8, 7][..].hash(h));
        let manual = digest(|h| {
            h.write(&3u64.to_le_bytes());
            h.write(&[9, 8, 7]);
        });
        assert_eq!(slice, manual);
    }
}
