#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: &[u8]| hidlins_local_sync_fuzz::state_input(input));
