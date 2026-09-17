use crate::payments::model::Minor;

/// Fee math with cent precision (PLAN #41: "комиссии: своя + эквайера,
/// точность до цента"). All inputs are percentages/amounts expressed in the
/// payment currency; results are rounded UP to the next minor unit (cents)
/// so the total the user pays is never under-quoted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fees {
    /// Our own service fee, in minor units.
    pub service: Minor,
    /// Acquirer fee (percent part + fixed part), in minor units.
    pub acquirer: Minor,
}

impl Fees {
    pub fn total(&self) -> Minor {
        self.service.saturating_add(self.acquirer)
    }
}

/// Compute service + acquirer fees for a gross amount in minor units.
///
/// * `service_percent` — our cut, flat percentage of the gross (e.g. 0.7).
/// * `acquirer_percent` — acquirer's percentage of the gross (route fee_percent).
/// * `acquirer_fixed` — acquirer's fixed per-payment fee, in minor units.
pub fn compute(gross: Minor, service_percent: f64, acquirer_percent: f64, acquirer_fixed: Minor) -> Fees {
    let service = percent_ceil(gross, service_percent);
    let acquirer = percent_ceil(gross, acquirer_percent)
        .saturating_add(acquirer_fixed);
    Fees { service, acquirer }
}

/// Amount the recipient actually gets: gross minus all fees.
pub fn net_amount(gross: Minor, fees: &Fees) -> Minor {
    gross.saturating_sub(fees.total()).max(0)
}

/// `percent` of `gross`, rounded up to the next cent.
pub fn percent_ceil(gross: Minor, percent: f64) -> Minor {
    if percent <= 0.0 {
        return 0;
    }
    (gross as f64 * percent / 100.0).ceil() as Minor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fees_round_up_to_the_cent() {
        // 0.7% of 1000.00 USD = 7.0000 → 700, no rounding.
        let f = compute(100_000, 0.7, 3.1, 50);
        assert_eq!(f.service, 700);
        assert_eq!(f.acquirer, 3_100 + 50);
        assert_eq!(f.total(), 3_850);
    }

    #[test]
    fn fees_never_round_down() {
        // 3.1% of 7 cents = 0.217 cents → 1 cent (ceil), never under-charge.
        let f = compute(7, 0.0, 3.1, 0);
        assert_eq!(f.acquirer, 1);
        assert_eq!(f.service, 0);
    }

    #[test]
    fn net_is_gross_minus_total() {
        let f = Fees { service: 50, acquirer: 100 };
        assert_eq!(net_amount(10_000, &f), 9_850);
    }

    #[test]
    fn net_never_negative() {
        let f = Fees { service: 5_000, acquirer: 5_000 };
        assert_eq!(net_amount(100, &f), 0);
    }

    #[test]
    fn zero_fees_are_zero() {
        let f = compute(999_999, 0.0, 0.0, 0);
        assert_eq!(f.total(), 0);
    }
}