//! Corrupted exact-region and account validation with checked lazy reads.
#![no_main]
#![forbid(unsafe_code)]

libfuzzer_sys::fuzz_target!(|data: &[u8]| blubber_fuzz::validate(data));
