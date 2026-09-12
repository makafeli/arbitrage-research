use primitive_types::{U256, U512};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{cmp::Ordering, fmt, str::FromStr};

const MAX_U256: &str =
    "115792089237316195423570985008687907853269984665640564039457584007913129639935";

/// Canonical decimal base units. JSON always uses strings, never floating point.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct AtomicAmount(String);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AmountError {
    NonCanonical,
    Overflow,
    Underflow,
    DivisionByZero,
    InexactRounding,
    InvalidDecimals,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rounding {
    Down,
    Up,
    Exact,
}

impl AtomicAmount {
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn is_zero(&self) -> bool {
        self.0 == "0"
    }
    pub fn zero() -> Self {
        Self("0".into())
    }
    fn integer(&self) -> U256 {
        U256::from_dec_str(&self.0).expect("validated amount invariant")
    }
    fn from_integer(value: U256) -> Self {
        Self(value.to_string())
    }
    pub fn checked_add(&self, rhs: &Self) -> Result<Self, AmountError> {
        self.integer()
            .checked_add(rhs.integer())
            .map(Self::from_integer)
            .ok_or(AmountError::Overflow)
    }
    pub fn checked_sub(&self, rhs: &Self) -> Result<Self, AmountError> {
        self.integer()
            .checked_sub(rhs.integer())
            .map(Self::from_integer)
            .ok_or(AmountError::Underflow)
    }
    pub fn checked_mul(&self, rhs: &Self) -> Result<Self, AmountError> {
        self.integer()
            .checked_mul(rhs.integer())
            .map(Self::from_integer)
            .ok_or(AmountError::Overflow)
    }
    pub fn checked_div(&self, denominator: &Self, rounding: Rounding) -> Result<Self, AmountError> {
        self.checked_mul_div(&Self::from(1), denominator, rounding)
    }
    /// Exact 512-bit intermediate prevents false overflow before division.
    /// Every caller explicitly selects rounding; this is not venue-specific quote math.
    pub fn checked_mul_div(
        &self,
        multiplier: &Self,
        denominator: &Self,
        rounding: Rounding,
    ) -> Result<Self, AmountError> {
        if denominator.is_zero() {
            return Err(AmountError::DivisionByZero);
        }
        let product = self.integer().full_mul(multiplier.integer());
        let divisor = U512::from(denominator.integer());
        let mut quotient = product / divisor;
        let remainder = product % divisor;
        if !remainder.is_zero() {
            match rounding {
                Rounding::Exact => return Err(AmountError::InexactRounding),
                Rounding::Up => {
                    quotient = quotient
                        .checked_add(U512::one())
                        .ok_or(AmountError::Overflow)?
                }
                Rounding::Down => {}
            }
        }
        if quotient > U512::from(U256::MAX) {
            return Err(AmountError::Overflow);
        }
        quotient.to_string().parse()
    }
    pub fn checked_rescale(
        &self,
        from: Decimals,
        to: Decimals,
        rounding: Rounding,
    ) -> Result<Self, AmountError> {
        let difference = from.get().abs_diff(to.get());
        let scale = format!("1{}", "0".repeat(usize::from(difference))).parse::<Self>()?;
        if to >= from {
            self.checked_mul(&scale)
        } else {
            self.checked_div(&scale, rounding)
        }
    }
}
impl From<u64> for AtomicAmount {
    fn from(value: u64) -> Self {
        Self(value.to_string())
    }
}
impl Ord for AtomicAmount {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0
            .len()
            .cmp(&other.0.len())
            .then_with(|| self.0.cmp(&other.0))
    }
}
impl PartialOrd for AtomicAmount {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
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
        if value.len() > MAX_U256.len() || (value.len() == MAX_U256.len() && value > MAX_U256) {
            return Err(AmountError::Overflow);
        }
        Ok(Self(value.to_owned()))
    }
}
impl Serialize for AtomicAmount {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
impl<'de> Deserialize<'de> for AtomicAmount {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}
impl fmt::Display for AtomicAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
impl fmt::Display for AmountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NonCanonical => "expected canonical decimal integer base units",
            Self::Overflow => "amount exceeds unsigned 256-bit range",
            Self::Underflow => "unsigned subtraction would be negative",
            Self::DivisionByZero => "denominator must be nonzero",
            Self::InexactRounding => "exact rounding requested but division has a remainder",
            Self::InvalidDecimals => "decimals must be an integer in 0..=255",
        })
    }
}
impl std::error::Error for AmountError {}

/// Signed P&L supports the full unsigned 256-bit magnitude in either direction.
/// Negative zero is rejected, so every value has exactly one representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedAmount {
    negative: bool,
    magnitude: AtomicAmount,
}
impl SignedAmount {
    pub fn difference(output: &AtomicAmount, input: &AtomicAmount) -> Self {
        if output >= input {
            Self {
                negative: false,
                magnitude: output.checked_sub(input).expect("ordered subtraction"),
            }
        } else {
            Self {
                negative: true,
                magnitude: input.checked_sub(output).expect("ordered subtraction"),
            }
        }
    }
    pub fn magnitude(&self) -> &AtomicAmount {
        &self.magnitude
    }
    pub fn is_negative(&self) -> bool {
        self.negative
    }
    pub fn checked_sub_cost(&self, cost: &AtomicAmount) -> Result<Self, AmountError> {
        if self.negative {
            Ok(Self {
                negative: true,
                magnitude: self.magnitude.checked_add(cost)?,
            })
        } else {
            Ok(Self::difference(&self.magnitude, cost))
        }
    }
}
impl FromStr for SignedAmount {
    type Err = AmountError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (negative, raw) = value
            .strip_prefix('-')
            .map_or((false, value), |raw| (true, raw));
        let magnitude: AtomicAmount = raw.parse()?;
        if negative && magnitude.is_zero() {
            return Err(AmountError::NonCanonical);
        }
        Ok(Self {
            negative,
            magnitude,
        })
    }
}
impl fmt::Display for SignedAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.negative {
            f.write_str("-")?;
        }
        self.magnitude.fmt(f)
    }
}
impl Serialize for SignedAmount {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
impl<'de> Deserialize<'de> for SignedAmount {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?.parse().map_err(de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u8")]
pub struct Decimals(u8);
impl Decimals {
    pub fn get(self) -> u8 {
        self.0
    }
}
impl TryFrom<u16> for Decimals {
    type Error = AmountError;
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        u8::try_from(value)
            .map(Self)
            .map_err(|_| AmountError::InvalidDecimals)
    }
}
impl From<Decimals> for u8 {
    fn from(value: Decimals) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wire_amounts_remain_strings() {
        for value in ["0", "9007199254740993", MAX_U256] {
            let parsed: AtomicAmount = value.parse().unwrap();
            assert_eq!(
                serde_json::to_string(&parsed).unwrap(),
                format!("\"{value}\"")
            );
            assert_eq!(
                serde_json::from_str::<AtomicAmount>(&format!("\"{value}\"")).unwrap(),
                parsed
            );
        }
        assert!(serde_json::from_str::<AtomicAmount>("9007199254740993").is_err());
    }
    #[test]
    fn malformed_and_overflow_rejected() {
        for value in ["", "00", "01", "-1", "+1", "1.0", "1e6", " 1", "１"] {
            assert_eq!(
                value.parse::<AtomicAmount>(),
                Err(AmountError::NonCanonical)
            );
        }
        assert!(format!("{MAX_U256}0").parse::<AtomicAmount>().is_err());
        assert_eq!(
            MAX_U256
                .parse::<AtomicAmount>()
                .unwrap()
                .checked_add(&1.into()),
            Err(AmountError::Overflow)
        );
        assert_eq!(
            AtomicAmount::zero().checked_sub(&1.into()),
            Err(AmountError::Underflow)
        );
    }
    #[test]
    fn full_width_multiply_divide_and_rounding() {
        let max: AtomicAmount = MAX_U256.parse().unwrap();
        assert_eq!(
            max.checked_mul_div(&max, &max, Rounding::Exact).unwrap(),
            max
        );
        assert_eq!(
            AtomicAmount::from(10)
                .checked_div(&3.into(), Rounding::Down)
                .unwrap(),
            3.into()
        );
        assert_eq!(
            AtomicAmount::from(10)
                .checked_div(&3.into(), Rounding::Up)
                .unwrap(),
            4.into()
        );
        assert_eq!(
            AtomicAmount::from(10).checked_div(&3.into(), Rounding::Exact),
            Err(AmountError::InexactRounding)
        );
        assert_eq!(
            max.checked_div(&0.into(), Rounding::Down),
            Err(AmountError::DivisionByZero)
        );
        assert_eq!(
            max.checked_mul_div(&max, &1.into(), Rounding::Down),
            Err(AmountError::Overflow)
        );
    }
    #[test]
    fn deterministic_arithmetic_properties() {
        // Exhaustive small-domain comparison with independently wider native arithmetic.
        for a in 0..128u64 {
            for b in 1..64u64 {
                let x = AtomicAmount::from(a);
                let y = AtomicAmount::from(b);
                assert_eq!(x.checked_add(&y).unwrap().checked_sub(&y).unwrap(), x);
                assert_eq!(
                    x.checked_mul_div(&y, &7.into(), Rounding::Down)
                        .unwrap()
                        .to_string(),
                    ((u128::from(a) * u128::from(b)) / 7).to_string()
                );
                assert_eq!(
                    x.checked_mul_div(&y, &7.into(), Rounding::Up)
                        .unwrap()
                        .to_string(),
                    (u128::from(a) * u128::from(b)).div_ceil(7).to_string()
                );
            }
        }
    }
    #[test]
    fn signed_losses_and_decimals_are_explicit() {
        let pnl = SignedAmount::difference(&100.into(), &103.into())
            .checked_sub_cost(&2.into())
            .unwrap();
        assert_eq!(serde_json::to_string(&pnl).unwrap(), "\"-5\"");
        assert!("-0".parse::<SignedAmount>().is_err());
        for json in ["-1", "256", "1.5", "\"18\""] {
            assert!(serde_json::from_str::<Decimals>(json).is_err());
        }
        assert_eq!(serde_json::from_str::<Decimals>("255").unwrap().get(), 255);
        assert_eq!(
            AtomicAmount::from(123).checked_rescale(Decimals(2), Decimals(1), Rounding::Exact),
            Err(AmountError::InexactRounding)
        );
    }
}
