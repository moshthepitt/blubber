//! Authenticated corrupt policy-program histories under ASan and the pinned SBF runtime.
#![no_main]
#![forbid(unsafe_code)]
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    blubber_fuzz::validate(data);
    blubber_fuzz::corrupt::check(data, false);
});
