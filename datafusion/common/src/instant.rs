// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

//! WASM-compatible `Instant` wrapper.

#[cfg(all(target_family = "wasm", not(feature = "dekopon-monotonic")))]
/// Browser-backed monotonic instant for ordinary WebAssembly consumers.
pub type Instant = web_time::Instant;

#[expect(clippy::disallowed_types)]
#[cfg(not(target_family = "wasm"))]
/// Native monotonic instant. This is only a type alias.
pub type Instant = std::time::Instant;

// The monotonic interface has no wall-clock or timer authority. Its origin
// belongs to the invocation, so an Instant must not survive that invocation.
#[cfg(all(target_family = "wasm", feature = "dekopon-monotonic"))]
mod host {
    use std::ops::{Add, Sub};
    use std::time::Duration;

    // Canonical ABI of the approved WIT `monotonic.now-nanos: func() -> u64`.
    // The embedding component declares this interface in its WIT world.
    #[link(wasm_import_module = "dekopon:clock/monotonic@1.1.0")]
    unsafe extern "C" {
        #[link_name = "now-nanos"]
        fn now_nanos() -> u64;
    }

    /// A monotonic tick since the invocation-owned host origin.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Instant(u64);

    impl Instant {
        /// Read the invocation's real host monotonic clock.
        pub fn now() -> Self {
            // SAFETY: the embedding component declares the exact WIT signature;
            // instantiation fails unless the invocation-scoped host supplies it.
            Self(unsafe { now_nanos() })
        }

        /// Duration since an earlier instant; panics if the other instant is later.
        pub fn duration_since(self, earlier: Self) -> Duration {
            self.checked_duration_since(earlier)
                .expect("earlier instant is later than self")
        }

        /// Checked duration since an earlier instant.
        pub fn checked_duration_since(self, earlier: Self) -> Option<Duration> {
            self.0.checked_sub(earlier.0).map(Duration::from_nanos)
        }

        /// Duration elapsed according to the invocation clock.
        pub fn elapsed(self) -> Duration {
            Self::now().duration_since(self)
        }

        /// Add a duration without wrapping the host tick counter.
        pub fn checked_add(self, duration: Duration) -> Option<Self> {
            let nanos = u64::try_from(duration.as_nanos()).ok()?;
            self.0.checked_add(nanos).map(Self)
        }

        /// Subtract a duration without wrapping the host tick counter.
        pub fn checked_sub(self, duration: Duration) -> Option<Self> {
            let nanos = u64::try_from(duration.as_nanos()).ok()?;
            self.0.checked_sub(nanos).map(Self)
        }
    }

    impl Add<Duration> for Instant {
        type Output = Self;

        fn add(self, rhs: Duration) -> Self {
            self.checked_add(rhs).expect("instant overflow")
        }
    }

    impl Sub<Duration> for Instant {
        type Output = Self;

        fn sub(self, rhs: Duration) -> Self {
            self.checked_sub(rhs).expect("instant underflow")
        }
    }

    impl Sub for Instant {
        type Output = Duration;

        fn sub(self, rhs: Self) -> Duration {
            self.duration_since(rhs)
        }
    }
}

#[cfg(all(target_family = "wasm", feature = "dekopon-monotonic"))]
pub use host::Instant;

/// Current UTC wall time. The opt-in component profile reads the invocation-only
/// host wall clock; ordinary native and browser behavior is unchanged.
pub fn utc_now() -> chrono::DateTime<chrono::Utc> {
    #[cfg(all(target_family = "wasm", feature = "dekopon-monotonic"))]
    {
        // Canonical ABI of `dekopon:clock/wall@1.1.0`'s
        // `now-unix-millis: func() -> u64`. No monotonic tick is used as UTC.
        #[link(wasm_import_module = "dekopon:clock/wall@1.1.0")]
        unsafe extern "C" {
            #[link_name = "now-unix-millis"]
            fn now_unix_millis() -> u64;
        }
        // SAFETY: the embedding declares this interface, and an invocation-only
        // host provides it. A missing host import fails component instantiation.
        utc_from_unix_millis(unsafe { now_unix_millis() })
    }
    #[cfg(not(all(target_family = "wasm", feature = "dekopon-monotonic")))]
    {
        chrono::Utc::now()
    }
}

#[cfg(any(test, all(target_family = "wasm", feature = "dekopon-monotonic")))]
fn utc_from_unix_millis(millis: u64) -> chrono::DateTime<chrono::Utc> {
    let millis = i64::try_from(millis).expect("host wall milliseconds exceed i64");
    chrono::DateTime::from_timestamp_millis(millis)
        .expect("host wall milliseconds outside supported UTC range")
}

#[cfg(test)]
mod tests {
    use super::utc_from_unix_millis;

    #[test]
    fn host_wall_millis_preserves_precision_and_rejects_overflow() {
        let now = utc_from_unix_millis(1_700_000_000_123);
        assert_eq!(now.timestamp(), 1_700_000_000);
        assert_eq!(now.timestamp_subsec_millis(), 123);
        assert!(std::panic::catch_unwind(|| utc_from_unix_millis(u64::MAX)).is_err());
    }
}
