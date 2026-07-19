//! Exact byte-count evidence for streamed diagnostic captures.

use std::{cmp::Ordering, fmt};

use serde::{Deserialize, Deserializer, Serialize};

use crate::CoreError;

/// An exact, non-negative count of bytes represented as canonical base-10 text on the wire.
///
/// Process stderr is a stream: its total length is not bounded by Rust's address-space-sized
/// `usize`. Serializing a canonical decimal string keeps the public fact exact on every target
/// architecture and avoids asking JSON consumers to preserve a large numeric literal precisely.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactByteCount(String);

impl ExactByteCount {
    pub(crate) fn zero() -> Self {
        Self("0".to_owned())
    }

    pub(crate) fn add_usize(&mut self, added: usize) {
        if added == 0 {
            return;
        }

        let right = added.to_string();
        let mut left_digits = self.0.bytes().rev();
        let mut right_digits = right.bytes().rev();
        let mut sum_reversed = Vec::with_capacity(self.0.len().max(right.len()) + 1);
        let mut carry = 0_u8;

        loop {
            let left = left_digits.next().map(|digit| digit - b'0');
            let right = right_digits.next().map(|digit| digit - b'0');
            let (left, right) = match (left, right) {
                (None, None) => break,
                (left, right) => (left.unwrap_or(0), right.unwrap_or(0)),
            };
            let total = left + right + carry;
            sum_reversed.push(b'0' + total % 10);
            carry = total / 10;
        }
        if carry != 0 {
            sum_reversed.push(b'0' + carry);
        }
        sum_reversed.reverse();
        self.0 = sum_reversed.into_iter().map(char::from).collect();
    }

    pub(crate) fn from_usize(value: usize) -> Self {
        Self(value.to_string())
    }

    pub(super) fn compare_usize(&self, value: usize) -> Ordering {
        let value = value.to_string();
        self.0
            .len()
            .cmp(&value.len())
            .then_with(|| self.0.cmp(&value))
    }

    fn validate(&self) -> Result<(), CoreError> {
        let bytes = self.0.as_bytes();
        if bytes.is_empty()
            || (bytes.len() > 1 && bytes[0] == b'0')
            || bytes.iter().any(|byte| !byte.is_ascii_digit())
        {
            return Err(CoreError::contract(
                "delivery byte counts must be canonical non-negative decimal strings",
            ));
        }
        Ok(())
    }

    /// Returns the canonical base-10 representation of this exact count.
    pub fn as_decimal(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ExactByteCount {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_decimal())
    }
}

impl Serialize for ExactByteCount {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ExactByteCount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let count = Self(String::deserialize(deserializer)?);
        count.validate().map_err(serde::de::Error::custom)?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::ExactByteCount;

    #[test]
    fn preserves_exactness_after_crossing_the_platform_usize_limit() {
        let mut count = ExactByteCount::from_usize(usize::MAX);
        count.add_usize(1);
        assert_eq!(count.as_decimal(), ((usize::MAX as u128) + 1).to_string());
    }

    #[test]
    fn zero_length_reads_are_identity_and_decimal_carry_grows_the_count() {
        let mut count = ExactByteCount::zero();
        count.add_usize(0);
        assert_eq!(count.as_decimal(), "0");

        count.add_usize(9);
        count.add_usize(1);
        assert_eq!(count.as_decimal(), "10");
    }

    #[test]
    fn rejects_noncanonical_or_lossy_wire_values() {
        for value in ["", "00", "01", "+1", "1.0", " 1"] {
            assert!(serde_json::from_value::<ExactByteCount>(serde_json::json!(value)).is_err());
        }
        assert!(serde_json::from_value::<ExactByteCount>(serde_json::json!(1)).is_err());
    }
}
