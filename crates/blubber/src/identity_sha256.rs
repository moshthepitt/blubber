//! Const SHA-256 for descriptor hashing. FIPS 180-4, section 6.2.

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

pub(crate) struct Sha256 {
    state: [u32; 8],
    block: [u8; 64],
    used: usize,
    length: u64,
}
impl Sha256 {
    pub(crate) const fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            block: [0; 64],
            used: 0,
            length: 0,
        }
    }
    const fn byte(&mut self, byte: u8) {
        self.block[self.used] = byte;
        self.used += 1;
        if self.used == 64 {
            self.compress();
            self.used = 0;
        }
    }
    pub(crate) const fn update(&mut self, bytes: &[u8]) {
        self.length = match self.length.checked_add(bytes.len() as u64) {
            Some(n) if n <= u64::MAX / 8 => n,
            _ => panic!("SHA-256 input too long"),
        };
        let mut i = 0;
        while i < bytes.len() {
            self.byte(bytes[i]);
            i += 1;
        }
    }
    pub(crate) const fn finish(mut self) -> [u8; 32] {
        let bits = (self.length * 8).to_be_bytes();
        self.byte(0x80);
        while self.used != 56 {
            self.byte(0);
        }
        let mut i = 0;
        while i < 8 {
            self.byte(bits[i]);
            i += 1;
        }
        let mut output = [0; 32];
        i = 0;
        while i < 8 {
            let bytes = self.state[i].to_be_bytes();
            let mut j = 0;
            while j < 4 {
                output[4 * i + j] = bytes[j];
                j += 1;
            }
            i += 1;
        }
        output
    }
    const fn compress(&mut self) {
        let mut w = [0u32; 64];
        let mut i = 0;
        while i < 16 {
            w[i] = u32::from_be_bytes([
                self.block[4 * i],
                self.block[4 * i + 1],
                self.block[4 * i + 2],
                self.block[4 * i + 3],
            ]);
            i += 1;
        }
        while i < 64 {
            let x = w[i - 15];
            let y = w[i - 2];
            let s0 = x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3);
            let s1 = y.rotate_right(17) ^ y.rotate_right(19) ^ (y >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
            i += 1;
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        i = 0;
        while i < 64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
            i += 1;
        }
        let work = [a, b, c, d, e, f, g, h];
        i = 0;
        while i < 8 {
            self.state[i] = self.state[i].wrapping_add(work[i]);
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Sha256;
    use sha2::{Digest, Sha256 as Reference};

    #[test]
    fn fips_vectors() {
        const EMPTY: [u8; 32] = Sha256::new().finish();
        const ABC: [u8; 32] = {
            let mut hash = Sha256::new();
            hash.update(b"abc");
            hash.finish()
        };
        const LONG: [u8; 32] = {
            let mut hash = Sha256::new();
            hash.update(b"abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmno");
            hash.update(b"ijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu");
            hash.finish()
        };
        for (input, digest) in [
            (&b""[..],EMPTY),
            (&b"abc"[..],ABC),
            (&b"abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu"[..],LONG),
        ] {
            assert_eq!(digest.as_slice(),Reference::digest(input).as_slice());
        }
        assert_eq!(
            ABC,
            [
                0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
                0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
                0xf2, 0x00, 0x15, 0xad
            ]
        );
    }
}
