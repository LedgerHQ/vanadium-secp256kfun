//! Scalar arithmetic (integers mod the secp256k1 group order)
use crate::{backend, hash::HashInto, marker::*, op};
use backend::BackendScalar;
use core::marker::PhantomData;
use digest::{generic_array::typenum::U32, Digest};
use rand_core::{CryptoRng, RngCore};

/// A secp256k1 scalar (an integer mod the curve order)
///
/// The term _scalar_ comes from interpreting the secp256k1 elliptic curve group
/// as a [_vector space_][4] with a point as _vectors_ and the field of integers
/// modulo the curve order as its scalars. Specifically, a `Scalar`
/// represents an integer modulo
///
/// 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
///
/// The thing that makes secp256k1 useful for cryptography is that scalar
/// _multiplication_ can be done efficiently:
///
/// ```
/// use secp256kfun::{g, Scalar, G};
/// let x = Scalar::random(&mut rand::thread_rng());
/// let X = g!(x * G);
/// ```
/// But finding `x` from `(X,G)` is hard (you can't divide `X` by `G`).
/// This is otherwise known as the elliptic curve [_discrete logarithm problem_
///
/// Because of this, scalars are often used as _secret keys_ and their
/// corresponding points are used as _public keys_.
///
/// [1]: https://en.wikipedia.org/wiki/One-way_function
/// [2]: https://en.wikipedia.org/wiki/Discrete_logarithm
/// [3]: https://en.wikipedia.org/wiki/Group_isomorphism
/// [4]: https://en.wikipedia.org/wiki/Vector_space
///
/// # Markers
///
///
/// A Scalar is marked whether it is known to not be zero
#[derive(Clone)]
pub struct Scalar<S = Secret, Z = NonZero>(pub(crate) backend::Scalar, PhantomData<(Z, S)>);

impl<Z, S> Scalar<S, Z> {
    /// Serializes the scalar to its 32-byte big-endian representation
    pub fn to_bytes(&self) -> [u8; 32] {
        backend::Scalar::to_bytes(&self.0)
    }

    /// Negates the scalar in-place if `cond` is true.
    pub fn conditional_negate(&mut self, cond: bool) {
        op::ScalarUnary::conditional_negate(self, cond)
    }

    /// Returns whether the scalar is greater than the `curve_order`/2.
    pub fn is_high(&self) -> bool {
        op::ScalarUnary::is_high(self)
    }

    /// Returns true if the scalar is equal to zero
    pub fn is_zero(&self) -> bool {
        op::ScalarUnary::is_zero(self)
    }

    pub(crate) fn from_inner(inner: backend::Scalar) -> Self {
        Scalar(inner, PhantomData)
    }
}

impl Scalar<Public, Zero> {
    /// Returns the zero scalar
    pub fn zero() -> Self {
        Self::from_inner(backend::Scalar::zero())
    }
}

impl<S> Scalar<S, NonZero> {
    /// Returns the multiplicative inverse of the scalar.
    pub fn invert(&self) -> Self {
        Self::from_inner(op::ScalarUnary::invert(self))
    }
}

impl Scalar<Secret, NonZero> {
    /// Generates a random scalar from randomness taken from a caller provided
    /// cryptographically secure random number generator.
    /// # Example
    /// ```
    /// use secp256kfun::{g, Scalar, G};
    /// let secret_scalar = Scalar::random(&mut rand::thread_rng());
    /// let public_point = g!(secret_scalar * G);
    /// ```
    pub fn random<R: RngCore + CryptoRng>(rng: &mut R) -> Self {
        let mut bytes = [0u8; 32];
        rng.fill_bytes(&mut bytes);
        Scalar::from_bytes_mod_order(bytes)
            .mark::<NonZero>()
            .expect("computationally unreachable")
    }
    /// Converts the output of a 32-byte hash into a scalar by reducing it modulo the curve order.
    /// # Example
    /// ```
    /// use digest::Digest;
    /// use secp256kfun::Scalar;
    /// let mut hash = sha2::Sha256::default();
    /// hash.input(b"Chancellor on brink of second bailout for banks".as_ref());
    /// let scalar = Scalar::from_hash(hash);
    /// assert_eq!(
    ///     scalar.to_bytes(),
    ///     hex_literal::hex!("8131e6f4b45754f2c90bd06688ceeabc0c45055460729928b4eecf11026a9e2d")
    /// );
    /// ```
    pub fn from_hash(hash: impl Digest<OutputSize = U32>) -> Self {
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(hash.result().as_slice());
        Scalar::from_bytes_mod_order(bytes)
            .mark::<NonZero>()
            .expect("computationally unreachable")
    }
}

impl Scalar<Secret, Zero> {
    /// Converts 32 bytes into a scalar by reducing it modulo the curve order.
    /// # Example
    /// ```
    /// use secp256kfun::Scalar;
    /// let scalar = Scalar::from_bytes_mod_order(*b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");
    /// assert_eq!(scalar.to_bytes(), *b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");
    /// let scalar_overflowed = Scalar::from_bytes_mod_order(hex_literal::hex!(
    ///     "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364142"
    /// ));
    /// assert_eq!(scalar_overflowed, Scalar::from(1))
    /// ```
    pub fn from_bytes_mod_order(bytes: [u8; 32]) -> Self {
        Self::from_inner(backend::Scalar::from_bytes_mod_order(bytes))
    }

    /// Exactly like [from_bytes_mod_order](Scalar::from_bytes_mod_order) except
    /// it operates on a 32-byte slice rather than an array.  If the slice is
    /// not 32 bytes long then the function returns `None`.
    ///
    /// # Example
    /// ```
    /// # use secp256kfun::Scalar;
    /// let bytes = b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
    /// assert!(Scalar::from_slice_mod_order(&bytes[..31]).is_none());
    /// assert_eq!(
    ///     Scalar::from_slice_mod_order(&bytes[..]).unwrap(),
    ///     Scalar::from_bytes_mod_order(*bytes)
    /// );
    /// ```
    pub fn from_slice_mod_order(slice: &[u8]) -> Option<Self> {
        if slice.len() != 32 {
            return None;
        }
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(slice);
        Some(Self::from_bytes_mod_order(bytes))
    }

    /// Creates a scalar from 32 big-endian encoded bytes. If the bytes
    /// represent an integer greater than or equal to the curve order then it returns `None`.
    pub fn from_bytes(bytes: [u8; 32]) -> Option<Self> {
        backend::Scalar::from_bytes(bytes).map(Self::from_inner)
    }

    /// Creates a scalar from 32 big-endian encoded bytes in a slice. If the
    /// length of the slice is not 32 or the bytes represent an integer greater
    /// than or equal to the curve order then it returns `None`.
    pub fn from_slice(slice: &[u8]) -> Option<Self> {
        if slice.len() != 32 {
            return None;
        }
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&slice);
        Self::from_bytes(bytes)
    }
}

impl Scalar<Public, NonZero> {
    pub fn from_non_zero_u32(int: core::num::NonZeroU32) -> Self {
        Self::from_inner(backend::Scalar::from_u32(int.get()))
    }

    pub fn one() -> Self {
        crate::nzscalar!(1)
    }

    pub fn minus_one() -> Self {
        Self::from_inner(backend::Scalar::minus_one())
    }
}

impl<Z1, Z2, S1, S2> PartialEq<Scalar<S2, Z2>> for Scalar<S1, Z1> {
    fn eq(&self, rhs: &Scalar<S2, Z2>) -> bool {
        crate::op::ScalarBinary::eq((self, rhs))
    }
}

impl From<u32> for Scalar<Public, Zero> {
    fn from(int: u32) -> Self {
        Self(backend::Scalar::from_u32(int), PhantomData)
    }
}

crate::impl_fromstr_deserailize! {
    name => "non-zero secp256k1 scalar",
    fn from_bytes<S: Secrecy>(bytes: [u8;32]) -> Option<Scalar<S,NonZero>> {
        Scalar::from_bytes(bytes).and_then(|scalar| scalar.mark::<S>().mark::<NonZero>())
    }
}

crate::impl_display_debug_serialize! {
    fn to_bytes<Z,S>(scalar: &Scalar<S,Z>) -> [u8;32] {
        scalar.to_bytes()
    }
}

crate::impl_fromstr_deserailize! {
    name => "secp256k1 scalar",
    fn from_bytes<S: Secrecy>(bytes: [u8;32]) -> Option<Scalar<S,Zero>> {
        Scalar::from_bytes(bytes).map(|scalar| scalar.mark::<S>())
    }
}

impl<S, Z> core::ops::Neg for Scalar<S, Z> {
    type Output = Scalar<S, Z>;

    fn neg(self) -> Self::Output {
        use crate::op::ScalarUnary;
        Scalar::from_inner(ScalarUnary::negate(&self))
    }
}

impl<S, Z> core::ops::Neg for &Scalar<S, Z> {
    type Output = Scalar<S, Z>;

    fn neg(self) -> Self::Output {
        use crate::op::ScalarUnary;
        Scalar::from_inner(ScalarUnary::negate(self))
    }
}

impl HashInto for Scalar {
    fn hash_into(&self, hash: &mut impl Digest) {
        hash.input(&self.to_bytes())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{nzscalar, op, s};

    crate::test_plus_wasm! {
        fn random() {
            let scalar_1 = Scalar::random(&mut rand::thread_rng());
            let scalar_2 = Scalar::random(&mut rand::thread_rng());
            assert_ne!(scalar_1, scalar_2);
        }

        fn invert() {
            let x = Scalar::random(&mut rand::thread_rng());
            assert!(s!(x  * {x.invert()}) == Scalar::from(1));
        }

        fn neg() {
            let x = Scalar::random(&mut rand::thread_rng());
            assert_eq!(s!(x - x), Scalar::zero());
            assert_eq!(-Scalar::zero(), Scalar::zero())
        }

        fn one() {
            assert_eq!(Scalar::one(), Scalar::from(1));
            assert_eq!(Scalar::minus_one(), -Scalar::one());
            assert_eq!(op::scalar_mul(&nzscalar!(3), &Scalar::minus_one()), -nzscalar!(3));
        }

        fn zero() {
            assert_eq!(Scalar::zero(), Scalar::from(0));
        }

        fn from_slice() {
            assert!(Scalar::from_slice(b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx".as_ref()).is_some());
            assert!(Scalar::from_slice(b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx".as_ref()).is_none());

            assert!(Scalar::from_slice(
                hex_literal::hex!("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")
                    .as_ref()
            )
                    .is_none());
        }

        fn from_slice_mod_order() {
            assert_eq!(
                Scalar::from_slice_mod_order(b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx".as_ref())
                    .unwrap()
                    .to_bytes(),
                *b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
            );

            assert_eq!(
                Scalar::from_slice_mod_order(
                    hex_literal::hex!(
                        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364142"
                    )
                        .as_ref()
                )
                    .unwrap(),
                Scalar::from(1)
            )
        }

        fn scalar_subtraction_is_not_commutative() {
            let two = Scalar::from(2);
            let three = Scalar::from(3);
            let minus_1 = Scalar::minus_one();
            let one = Scalar::from(1);

            assert_eq!(
                minus_1,
                Scalar::from_bytes_mod_order(hex_literal::hex!(
                    "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364140"
                ))
            );
            assert_eq!(s!(two - three), minus_1);
            assert_eq!(s!(three - two), one);
        }


        fn nz_scalar_to_scalar_subtraction_is_not_commutative() {
            let two = nzscalar!(2);
            let three = nzscalar!(3);
            let minus_1 = Scalar::minus_one();
            let one = Scalar::from(1);

            assert_eq!(s!(two - three), minus_1);
            assert_eq!(s!(three - two), one);
        }
    }
}
