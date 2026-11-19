//! Ordinary consumer of erased library proofs and generic marker selection.
#![no_std]
#![forbid(unsafe_code)]

use blubber::__private::{encoder_route, EncoderRoute};
use blubber::Schema;
use blubber_core::encoding::{bytes, encode_frame, encode_pair};

/// Generated graph with an unchanged borrowed initializer shape.
#[blubber::schema]
pub struct ConsumerGraph<T: Schema> {
    /// Optional counted child values.
    pub values: Option<blubber::List<T>>,
}

/// Native fixed projection compiled through the ordinary consumer toolchains.
#[derive(Clone, Copy, blubber::Fixed)]
pub struct Packed {
    /// Restricted one-byte field.
    pub flag: bool,
    /// Unaligned packed integer.
    pub count: u32,
}

/// Selects a route using only the existing public Schema bound.
pub const fn generic_route<T: Schema>() -> EncoderRoute {
    encoder_route::<T>()
}

/// Monomorphic compile-time selection through a generic graph and alias.
pub const BUILTIN_ROUTE: EncoderRoute = generic_route::<ConsumerGraph<u32>>();

/// Exercises both production generic drivers after spec erasure.
///
/// # Panics
/// The destination length must equal the source length.
pub fn erased_borrowed_drivers(source: &[u8], destination: &mut [u8]) {
    let left = bytes(source);
    let empty = bytes(&[]);
    encode_pair(&left, &empty, destination);
    encode_frame(&left, destination, 0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_and_projected_init_need_no_verifier() {
        type Alias = u32;
        assert_eq!(BUILTIN_ROUTE, EncoderRoute::Contract);
        assert_eq!(
            generic_route::<ConsumerGraph<Alias>>(),
            EncoderRoute::Contract
        );
        let input = [3u32, 5];
        let init = ConsumerGraphInit::<Alias> {
            values: Some(&input),
        };
        assert_eq!(ConsumerGraph::<Alias>::measure(&init), Ok(13));
        let mut graph = [0x19; 15];
        ConsumerGraph::<Alias>::encode(&init, &mut graph[..13]);
        assert_eq!(graph, [1, 2, 0, 0, 0, 3, 0, 0, 0, 5, 0, 0, 0, 0x19, 0x19]);
        assert_eq!(
            generic_route::<ConsumerGraph<Packed>>(),
            EncoderRoute::Contract
        );
        assert_eq!(
            <Packed as blubber::Fixed>::encode(Packed {
                flag: true,
                count: 0x0102_0304
            }),
            [1, 4, 3, 2, 1]
        );
        let mut output = [0; 3];
        erased_borrowed_drivers(&[3, 5, 7], &mut output);
        assert_eq!(output, [3, 5, 7]);
    }
}
