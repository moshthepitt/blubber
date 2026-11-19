//! Bounded edits of arbitrary and independently encoded account states.
#![no_main]
#![forbid(unsafe_code)]

libfuzzer_sys::fuzz_target!(|data: &[u8]| blubber_fuzz::edit(data));
