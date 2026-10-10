use super::error::{invalid, Result};

pub(crate) fn units(value: &str, precision: u32) -> Result<u128> {
    if value.is_empty() || value.len() > 80 || precision > 18 {
        return Err(invalid("invalid amount"));
    }
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
        || fraction.len() > precision as usize
        || value.ends_with('.')
    {
        return Err(invalid(
            "amount must be an exact positive decimal within asset precision",
        ));
    }
    let scale = 10_u128.pow(precision);
    let whole = whole
        .parse::<u128>()
        .map_err(|_| invalid("amount overflow"))?;
    let fraction = if fraction.is_empty() {
        0
    } else {
        fraction
            .parse::<u128>()
            .map_err(|_| invalid("amount overflow"))?
            * 10_u128.pow(precision - fraction.len() as u32)
    };
    whole
        .checked_mul(scale)
        .and_then(|n| n.checked_add(fraction))
        .filter(|n| *n > 0)
        .ok_or_else(|| invalid("amount overflow or zero"))
}
#[cfg(test)]
pub(crate) fn decimal(value: u128, precision: u32) -> String {
    let scale = 10_u128.pow(precision);
    if precision == 0 {
        return value.to_string();
    }
    let fraction = format!("{:0width$}", value % scale, width = precision as usize);
    let fraction = fraction.trim_end_matches('0');
    if fraction.is_empty() {
        (value / scale).to_string()
    } else {
        format!("{}.{}", value / scale, fraction)
    }
}
pub(crate) fn fee(basis: u128, precision: u32) -> Result<u128> {
    let percentage = basis / 400 + u128::from(basis % 400 != 0);
    let floor = 5_u128
        .checked_mul(
            10_u128
                .checked_pow(precision)
                .ok_or_else(|| invalid("fee precision overflow"))?,
        )
        .ok_or_else(|| invalid("fee overflow"))?;
    Ok(percentage.max(floor))
}
pub(crate) fn integer(value: &str) -> Result<u128> {
    value.parse().map_err(|_| invalid("invalid integer amount"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_decimal_conversion() {
        assert_eq!(units("1.000001", 6).unwrap(), 1_000_001);
        for value in ["0", "-1", "1e3", "NaN", "1.", ".1", "1.0000001", " 1"] {
            assert!(units(value, 6).is_err(), "{value}");
        }
        assert_eq!(decimal(1_000_001, 6), "1.000001");
        assert!(units(&u128::MAX.to_string(), 6).is_err());
    }
    #[test]
    fn fee_floor_and_single_rounding() {
        for (basis, expected) in [
            (100_000_000, 5_000_000),
            (1_000_000_000, 5_000_000),
            (2_000_000_000, 5_000_000),
            (2_000_000_001, 5_000_001),
            (10_000_000_000, 25_000_000),
        ] {
            assert_eq!(fee(basis, 6).unwrap(), expected);
        }
        assert_eq!(fee(u128::MAX, 6).unwrap(), u128::MAX / 400 + 1);
    }
}

pub(crate) fn units_or_zero(value: &str, precision: u32) -> Result<u128> {
    if value == "0" {
        Ok(0)
    } else {
        units(value, precision)
    }
}
