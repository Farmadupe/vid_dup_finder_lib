use crate::definitions::TOLERANCE_SCALING_FACTOR;

/// The distance between two [VideoHash][crate::VideoHash] objects.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct RawDistance {
    pub distance: u32,
}

impl RawDistance {
    pub fn within_tolerance(&self, tolerance: RawTolerance) -> bool {
        self.distance <= tolerance.value
    }

    pub fn u32_value(&self) -> u32 {
        self.distance
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct RawTolerance {
    pub value: u32,
}

impl From<&NormalizedTolerance> for RawTolerance {
    fn from(tol: &NormalizedTolerance) -> Self {
        Self {
            value: (tol.value * TOLERANCE_SCALING_FACTOR) as u32,
        }
    }
}

impl RawTolerance {
    pub fn contains(&self, dist: &RawDistance) -> bool {
        dist.distance <= self.value
    }
}

/// The distance between two VideoHashes, in the range 0..=1
#[derive(Clone, PartialEq, PartialOrd, Debug, Default)]
pub struct NormalizedDistance {
    distance: f64,
}

impl NormalizedDistance {
    pub fn new(distance: f64) -> Self {
        Self { distance }
    }

    pub fn value(&self) -> f64 {
        self.distance
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
/// Tolerance to be applied when searching for duplicates.
///
/// Formed of independent tolerances in the spatial domain (allowable difference in the shapes of each frame) and
/// in the temporal domain (allowable differences in movement between frames)
///
/// Spatial and temporal tolerances are specified as floating point numbers in the inclusive range (0..1).
/// The higher the number, the more two hashes may differ before they are no longer considered a match.
/// A tolerance of 0 implies two videos will only match if the raw bits of their respective hashes are identical.
/// A tolerance of 1 implies two videos will always match no matter the value of their hashes.
///
/// Depending on requirements, useful tolerances appear to be in the range (0.0..0.15).
pub struct NormalizedTolerance {
    value: f64,
}

impl NormalizedTolerance {
    ///Create a new Tolerance from spatial and temporal values.
    /// # Arguments
    /// * spatial: Spatial tolerance in the inclusive range (0..1)
    /// * temporal: Temporal tolerance in the inclusive range (0..1)
    pub fn new(value: f64) -> Self {
        assert!(value >= 0.0);
        assert!(value <= 1.0);

        Self { value }
    }

    pub fn value(&self) -> f64 {
        self.value
    }
}

impl Default for NormalizedTolerance {
    fn default() -> Self {
        Self { value: 0.08 }
    }
}
