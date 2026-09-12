use std::{fmt, str::FromStr};

const MAX_U256: &str =
    "115792089237316195423570985008687907853269984665640564039457584007913129639935";

/// Canonical unsigned decimal base units, preserving all 256 bits across JSON.
/// This is a boundary value, not a protocol math implementation.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct AtomicAmount(String);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AmountError {
    NonCanonical,
    Overflow,
}

impl AtomicAmount {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for AtomicAmount {
    type Err = AmountError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.is_empty()
            || !value.bytes().all(|byte| byte.is_ascii_digit())
            || (value.len() > 1 && value.starts_with('0'))
        {
            return Err(AmountError::NonCanonical);
        }
        if value.len() > MAX_U256.len()
            || (value.len() == MAX_U256.len() && value > MAX_U256)
        {
            return Err(AmountError::Overflow);
        }
        Ok(Self(value.to_owned()))
    }
}

impl fmt::Display for AtomicAmount {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Display for AmountError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NonCanonical => "expected canonical unsigned decimal base units",
            Self::Overflow => "amount exceeds unsigned 256-bit range",
        })
    }
}

impl std::error::Error for AmountError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_values_beyond_javascript_integer_precision() {
        for value in ["0", "9007199254740993", MAX_U256] {
            assert_eq!(value.parse::<AtomicAmount>().unwrap().as_str(), value);
        }
    }

    #[test]
    fn rejects_overflow_without_float_conversion() {
        let overflow =
            "115792089237316195423570985008687907853269984665640564039457584007913129639936";
        assert_eq!(overflow.parse::<AtomicAmount>(), Err(AmountError::Overflow));
    }

    #[test]
    fn rejects_ambiguous_or_noninteger_wire_amounts() {
        for value in ["", "00", "01", "-1", "+1", "1.0", "1e6", " 1", "１"] {
            assert_eq!(value.parse::<AtomicAmount>(), Err(AmountError::NonCanonical));
        }
    }
}
