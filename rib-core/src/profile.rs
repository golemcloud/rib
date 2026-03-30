// Copyright 2024-2025 Golem Cloud
//
// Licensed under the Golem Source License v1.0 (the "License");
// you may not use this file except in compliance with the License.
//
// Wall-clock sections for coarse profiling. Enable with:
//   RIB_PROFILE=1   or   RIB_PROFILE=true
//
// Timings print to stderr as `[rib-profile] label ...`.

use std::time::Instant;

pub(crate) fn rib_profile_enabled() -> bool {
    std::env::var_os("RIB_PROFILE").is_some_and(|v| {
        v.eq_ignore_ascii_case("1".as_ref())
            || v.eq_ignore_ascii_case("true".as_ref())
            || v.eq_ignore_ascii_case("yes".as_ref())
    })
}

/// On drop, prints elapsed time if `RIB_PROFILE` is set (inner scopes finish first).
pub(crate) struct Scope {
    label: &'static str,
    start: Instant,
    active: bool,
}

impl Scope {
    pub fn new(label: &'static str) -> Self {
        let active = rib_profile_enabled();
        Self {
            label,
            start: Instant::now(),
            active,
        }
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        if self.active {
            eprintln!("[rib-profile] {:<55} {:?}", self.label, self.start.elapsed());
        }
    }
}
