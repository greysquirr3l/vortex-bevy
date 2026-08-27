//! Bounded synchronous resource admission for conversion work.

use std::{
    num::NonZeroUsize,
    sync::{Mutex, MutexGuard},
};

use thiserror::Error;

/// Immutable limits applied before conversion work reserves resources.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConversionResourceLimits {
    max_in_flight: NonZeroUsize,
    max_reserved_bytes: NonZeroUsize,
}

impl ConversionResourceLimits {
    /// Creates non-zero limits for synchronous conversion admission.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionResourceError::ZeroLimit`] when either limit is zero.
    pub fn new(
        max_in_flight: usize,
        max_reserved_bytes: usize,
    ) -> Result<Self, ConversionResourceError> {
        let max_in_flight = NonZeroUsize::new(max_in_flight)
            .ok_or(ConversionResourceError::ZeroLimit { field: "max_in_flight" })?;
        let max_reserved_bytes = NonZeroUsize::new(max_reserved_bytes)
            .ok_or(ConversionResourceError::ZeroLimit { field: "max_reserved_bytes" })?;
        Ok(Self { max_in_flight, max_reserved_bytes })
    }

    /// Returns the maximum number of conversion operations admitted simultaneously.
    #[must_use]
    pub const fn max_in_flight(self) -> NonZeroUsize {
        self.max_in_flight
    }

    /// Returns the maximum total estimated bytes admitted simultaneously.
    #[must_use]
    pub const fn max_reserved_bytes(self) -> NonZeroUsize {
        self.max_reserved_bytes
    }
}

/// Admission or accounting failures for conversion resource governance.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ConversionResourceError {
    /// A requested configuration bound was zero.
    #[error("{field} must be greater than zero")]
    ZeroLimit {
        /// The invalid configuration field.
        field: &'static str,
    },
    /// One operation exceeds the configured aggregate memory budget alone.
    #[error("estimated conversion reservation of {requested} bytes exceeds the {limit}-byte limit")]
    RequestExceedsMemoryLimit {
        /// The requested estimated reservation.
        requested: usize,
        /// The configured aggregate byte limit.
        limit: usize,
    },
    /// The configured concurrent-operation or aggregate-memory capacity is currently occupied.
    #[error("conversion resource capacity is exhausted")]
    CapacityExhausted,
    /// Internal admission state was poisoned by a prior unwinding thread.
    #[error("conversion resource state is unavailable")]
    StateUnavailable,
}

#[derive(Debug)]
struct ResourceState {
    in_flight: usize,
    reserved_bytes: usize,
}

/// Thread-safe bounded admission controller for synchronous conversion operations.
#[derive(Debug)]
pub struct ConversionResourceGovernor {
    limits: ConversionResourceLimits,
    state: Mutex<ResourceState>,
}

impl ConversionResourceGovernor {
    /// Creates a governor for the supplied non-zero resource limits.
    #[must_use]
    pub const fn new(limits: ConversionResourceLimits) -> Self {
        Self { limits, state: Mutex::new(ResourceState { in_flight: 0, reserved_bytes: 0 }) }
    }

    /// Attempts to reserve the estimated memory and one in-flight slot.
    ///
    /// The returned permit releases its reservation on drop. This API is synchronous and never
    /// exposes a mutex guard, so a guard cannot cross an await point.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionResourceError::RequestExceedsMemoryLimit`] when one request exceeds
    /// the configured memory budget. Returns [`ConversionResourceError::CapacityExhausted`] when
    /// existing work owns the requested capacity. Returns [`ConversionResourceError::StateUnavailable`]
    /// when a prior caller poisoned internal accounting state.
    pub fn acquire(
        &self,
        estimated_bytes: usize,
    ) -> Result<ConversionResourcePermit<'_>, ConversionResourceError> {
        let max_reserved_bytes = self.limits.max_reserved_bytes.get();
        if estimated_bytes > max_reserved_bytes {
            return Err(ConversionResourceError::RequestExceedsMemoryLimit {
                requested: estimated_bytes,
                limit: max_reserved_bytes,
            });
        }
        let mut state = self.lock_state()?;
        let has_slot = state.in_flight < self.limits.max_in_flight.get();
        let available_bytes = max_reserved_bytes.saturating_sub(state.reserved_bytes);
        if !has_slot || estimated_bytes > available_bytes {
            return Err(ConversionResourceError::CapacityExhausted);
        }
        state.in_flight = state.in_flight.saturating_add(1);
        state.reserved_bytes = state.reserved_bytes.saturating_add(estimated_bytes);
        drop(state);
        Ok(ConversionResourcePermit { governor: self, reserved_bytes: estimated_bytes })
    }

    fn lock_state(&self) -> Result<MutexGuard<'_, ResourceState>, ConversionResourceError> {
        self.state.lock().map_err(|_| ConversionResourceError::StateUnavailable)
    }
}

/// Scoped reservation returned by [`ConversionResourceGovernor::acquire`].
#[derive(Debug)]
pub struct ConversionResourcePermit<'a> {
    governor: &'a ConversionResourceGovernor,
    reserved_bytes: usize,
}

impl Drop for ConversionResourcePermit<'_> {
    fn drop(&mut self) {
        if let Ok(mut state) = self.governor.state.lock() {
            state.in_flight = state.in_flight.saturating_sub(1);
            state.reserved_bytes = state.reserved_bytes.saturating_sub(self.reserved_bytes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ConversionResourceError, ConversionResourceGovernor, ConversionResourceLimits};

    #[test]
    fn limits_reject_zero_values_and_pathological_reservations()
    -> Result<(), Box<dyn std::error::Error>> {
        assert!(matches!(
            ConversionResourceLimits::new(0, 1024),
            Err(ConversionResourceError::ZeroLimit { field: "max_in_flight" })
        ));
        let limits = ConversionResourceLimits::new(1, 1024)?;
        let governor = ConversionResourceGovernor::new(limits);
        assert!(matches!(
            governor.acquire(1025),
            Err(ConversionResourceError::RequestExceedsMemoryLimit {
                requested: 1025,
                limit: 1024
            })
        ));
        Ok(())
    }

    #[test]
    fn permit_bounds_concurrency_and_releases_capacity() -> Result<(), Box<dyn std::error::Error>> {
        let governor = ConversionResourceGovernor::new(ConversionResourceLimits::new(1, 64)?);
        let reservation = governor.acquire(64)?;
        assert!(matches!(governor.acquire(1), Err(ConversionResourceError::CapacityExhausted)));
        drop(reservation);
        let released_reservation = governor.acquire(64)?;
        drop(released_reservation);
        Ok(())
    }
}
