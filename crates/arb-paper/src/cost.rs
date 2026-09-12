use arb_domain::{AssetId, AtomicAmount, NetworkId, Rounding, SignedAmount};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fmt};

/// Native fee inventory is distinct from wrapped tokens and starting principal.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "identity",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum AccountingAsset {
    Token(AssetId),
    Native(NetworkId),
}
impl AccountingAsset {
    pub fn network(&self) -> NetworkId {
        match self {
            Self::Token(a) => a.network(),
            Self::Native(n) => *n,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExpenseKind {
    NetworkExecution,
    BaseL1Data,
    PriorityFee,
    RelayTip,
    Funding,
    AccountSetup,
    Other,
}
/// Quote-included DEX fees and impact are informational, never an expense to
/// subtract again. The quote producer must supply exact venue mathematics.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quote {
    pub starting_asset: AssetId,
    pub amount_in: AtomicAmount,
    pub amount_out: AtomicAmount,
    pub includes_pool_fees_and_impact: bool,
    pub quote_reference: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum Valuation {
    /// Valid only when the expense currency already is the starting asset.
    SameAsset {
        reference: String,
        valued_at_unix_ms: u64,
    },
    /// Numerator starting-asset base units per denominator expense-asset base units.
    /// Decimals are already part of that declared ratio. Costs round upward.
    Ratio {
        numerator: AtomicAmount,
        denominator: AtomicAmount,
        reference: String,
        valued_at_unix_ms: u64,
    },
    Missing {
        reason: String,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum ExpenseAmount {
    Known {
        asset: AccountingAsset,
        amount: AtomicAmount,
        valuation: Valuation,
    },
    Missing {
        reason: String,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expense {
    pub kind: ExpenseKind,
    pub amount: ExpenseAmount,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum FundingAssumption {
    OwnVirtualCapital,
    Unknown { reason: String },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum OverheadAllocation {
    NotAllocated,
    Allocated {
        amount_in_start_asset: AtomicAmount,
        method: String,
        version: String,
        reference: String,
    },
    Unknown {
        reason: String,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ValuedExpense {
    pub expense: Expense,
    pub in_start_asset: Option<AtomicAmount>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CostReport {
    pub starting_asset: AssetId,
    pub gross_after_quote_included_costs: SignedAmount,
    pub transaction_net: Option<SignedAmount>,
    pub fully_allocated_net: Option<SignedAmount>,
    pub expenses: Vec<ValuedExpense>,
    pub overhead: OverheadAllocation,
    pub incomplete_reasons: Vec<String>,
}
impl CostReport {
    pub fn costs_complete(&self) -> bool {
        self.transaction_net.is_some() && self.incomplete_reasons.is_empty()
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaperError {
    pub field: &'static str,
    pub reason: &'static str,
}
impl fmt::Display for PaperError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.reason)
    }
}
impl std::error::Error for PaperError {}
pub(crate) fn paper_error(field: &'static str, reason: &'static str) -> PaperError {
    PaperError { field, reason }
}

pub fn evaluate_costs(
    quote: &Quote,
    expenses: &[Expense],
    funding: FundingAssumption,
    overhead: OverheadAllocation,
) -> Result<CostReport, PaperError> {
    if quote.amount_in.is_zero()
        || !quote.includes_pool_fees_and_impact
        || quote.quote_reference.trim().is_empty()
    {
        return Err(paper_error(
            "quote",
            "positive input, exact quote reference and already-included DEX fees/impact are required",
        ));
    }
    let mut seen = HashSet::new();
    let mut missing = Vec::new();
    let mut valued = Vec::new();
    let gross = SignedAmount::difference(&quote.amount_out, &quote.amount_in);
    let mut net = gross.clone();
    for expense in expenses {
        if !seen.insert(expense.kind) {
            return Err(paper_error(
                "expenses",
                "a cost component cannot be counted twice",
            ));
        }
        let converted = match &expense.amount {
            ExpenseAmount::Missing { reason } => {
                missing.push(format!("{:?}: {reason}", expense.kind));
                None
            }
            ExpenseAmount::Known {
                asset,
                amount,
                valuation,
            } => {
                if asset.network() != quote.starting_asset.network() {
                    return Err(paper_error(
                        "expenses.asset",
                        "cross-network fee valuation is unsupported",
                    ));
                }
                match valuation {
                    Valuation::Missing { reason } => {
                        missing.push(format!("{:?} valuation: {reason}", expense.kind));
                        None
                    }
                    Valuation::SameAsset {
                        reference,
                        valued_at_unix_ms,
                    } => {
                        if asset != &AccountingAsset::Token(quote.starting_asset.clone())
                            || reference.trim().is_empty()
                            || *valued_at_unix_ms == 0
                        {
                            return Err(paper_error(
                                "expenses.valuation",
                                "same-asset valuation requires identical currency and timestamped provenance",
                            ));
                        }
                        Some(amount.clone())
                    }
                    Valuation::Ratio {
                        numerator,
                        denominator,
                        reference,
                        valued_at_unix_ms,
                    } => {
                        if numerator.is_zero()
                            || denominator.is_zero()
                            || reference.trim().is_empty()
                            || *valued_at_unix_ms == 0
                        {
                            return Err(paper_error(
                                "expenses.valuation",
                                "conversion requires positive exact ratio and timestamped provenance",
                            ));
                        }
                        Some(
                            amount
                                .checked_mul_div(numerator, denominator, Rounding::Up)
                                .map_err(|_| {
                                    paper_error(
                                        "expenses.valuation",
                                        "conversion exceeds the supported exact amount range",
                                    )
                                })?,
                        )
                    }
                }
            }
        };
        if let Some(cost) = &converted {
            net = net.checked_sub_cost(cost).map_err(|_| {
                paper_error(
                    "transaction_net",
                    "signed net magnitude exceeds supported range",
                )
            })?;
        }
        valued.push(ValuedExpense {
            expense: expense.clone(),
            in_start_asset: converted,
        });
    }
    let required: &[ExpenseKind] = match quote.starting_asset.network() {
        NetworkId::BaseMainnet => &[ExpenseKind::NetworkExecution, ExpenseKind::BaseL1Data],
        NetworkId::SolanaMainnet => &[
            ExpenseKind::NetworkExecution,
            ExpenseKind::PriorityFee,
            ExpenseKind::RelayTip,
        ],
    };
    for kind in required {
        if !seen.contains(kind) {
            missing.push(format!(
                "{kind:?}: missing scenario input (use explicit known zero if not applicable)"
            ));
        }
    }
    if let FundingAssumption::Unknown { reason } = funding {
        missing.push(format!("funding assumption: {reason}"));
    }
    let transaction_net = if missing.is_empty() { Some(net) } else { None };
    let fully_allocated_net = match &overhead {
        OverheadAllocation::NotAllocated | OverheadAllocation::Unknown { .. } => None,
        OverheadAllocation::Allocated {
            amount_in_start_asset,
            method,
            version,
            reference,
        } => {
            if method.trim().is_empty() || version.trim().is_empty() || reference.trim().is_empty()
            {
                return Err(paper_error(
                    "overhead",
                    "allocation requires disclosed method, version and reference",
                ));
            }
            transaction_net
                .as_ref()
                .map(|n| {
                    n.checked_sub_cost(amount_in_start_asset).map_err(|_| {
                        paper_error(
                            "overhead",
                            "allocated net exceeds supported signed magnitude",
                        )
                    })
                })
                .transpose()?
        }
    };
    Ok(CostReport {
        starting_asset: quote.starting_asset.clone(),
        gross_after_quote_included_costs: gross,
        transaction_net,
        fully_allocated_net,
        expenses: valued,
        overhead,
        incomplete_reasons: missing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accounting_asset_rejects_unknown_fields() {
        let ordinary = serde_json::json!({"kind":"NATIVE","identity":"base-mainnet"});
        let parsed: AccountingAsset = serde_json::from_value(ordinary.clone()).unwrap();
        assert_eq!(parsed, AccountingAsset::Native(NetworkId::BaseMainnet));
        assert_eq!(serde_json::to_value(parsed).unwrap(), ordinary);
        let hidden = serde_json::json!({"kind":"NATIVE","identity":"base-mainnet","hidden":true});
        assert!(serde_json::from_value::<AccountingAsset>(hidden).is_err());
    }

    fn base_asset() -> AssetId {
        AssetId::new(
            NetworkId::BaseMainnet,
            "0x0000000000000000000000000000000000000001",
        )
        .unwrap()
    }
    fn solana_asset() -> AssetId {
        AssetId::new(
            NetworkId::SolanaMainnet,
            "So11111111111111111111111111111111111111112",
        )
        .unwrap()
    }
    fn fee(network: NetworkId, kind: ExpenseKind, amount: u64, num: u64, den: u64) -> Expense {
        Expense {
            kind,
            amount: ExpenseAmount::Known {
                asset: AccountingAsset::Native(network),
                amount: amount.into(),
                valuation: Valuation::Ratio {
                    numerator: num.into(),
                    denominator: den.into(),
                    reference: "synthetic-valuation-1".into(),
                    valued_at_unix_ms: 1_000,
                },
            },
        }
    }
    #[test]
    fn base_costs_reproduce_exact_native_conversions_and_disclosed_overhead() {
        let quote = Quote {
            starting_asset: base_asset(),
            amount_in: 100_000_000.into(),
            amount_out: 100_060_000.into(),
            includes_pool_fees_and_impact: true,
            quote_reference: "synthetic-pool-fees-already-included".into(),
        };
        let expenses = [
            fee(
                NetworkId::BaseMainnet,
                ExpenseKind::NetworkExecution,
                10_000_000_000_000,
                3_000_000_000,
                1_000_000_000_000_000_000,
            ),
            fee(
                NetworkId::BaseMainnet,
                ExpenseKind::BaseL1Data,
                1_000_000_000_000,
                3_000_000_000,
                1_000_000_000_000_000_000,
            ),
        ];
        let report = evaluate_costs(
            &quote,
            &expenses,
            FundingAssumption::OwnVirtualCapital,
            OverheadAllocation::Allocated {
                amount_in_start_asset: 50_000.into(),
                method: "equal-per-evaluated-attempt".into(),
                version: "1".into(),
                reference: "synthetic-overhead-run".into(),
            },
        )
        .unwrap();
        assert_eq!(report.gross_after_quote_included_costs.to_string(), "60000");
        assert_eq!(
            report.transaction_net.as_ref().unwrap().to_string(),
            "27000"
        );
        assert_eq!(
            report.fully_allocated_net.as_ref().unwrap().to_string(),
            "-23000"
        );
        assert!(report.costs_complete());
        assert_eq!(report.starting_asset, quote.starting_asset); // No guaranteed USD conversion exists in this report.
        let json = serde_json::to_value(report).unwrap();
        assert_eq!(json["expenses"][0]["in_start_asset"], "30000");
        assert_eq!(json["overhead"]["version"], "1");
    }
    #[test]
    fn solana_priority_and_tip_keep_losing_scenarios_visible() {
        let quote = Quote {
            starting_asset: solana_asset(),
            amount_in: 100_000.into(),
            amount_out: 101_000.into(),
            includes_pool_fees_and_impact: true,
            quote_reference: "synthetic-solana".into(),
        };
        let expenses = [
            fee(
                NetworkId::SolanaMainnet,
                ExpenseKind::NetworkExecution,
                5000,
                150_000_000,
                1_000_000_000,
            ),
            fee(
                NetworkId::SolanaMainnet,
                ExpenseKind::PriorityFee,
                1000,
                150_000_000,
                1_000_000_000,
            ),
            fee(
                NetworkId::SolanaMainnet,
                ExpenseKind::RelayTip,
                1000,
                150_000_000,
                1_000_000_000,
            ),
        ];
        let report = evaluate_costs(
            &quote,
            &expenses,
            FundingAssumption::OwnVirtualCapital,
            OverheadAllocation::NotAllocated,
        )
        .unwrap();
        assert_eq!(report.transaction_net.unwrap().to_string(), "-50");
        assert!(report.fully_allocated_net.is_none());
        let failed = Quote {
            amount_out: quote.amount_in.clone(),
            ..quote
        };
        let report = evaluate_costs(
            &failed,
            &expenses,
            FundingAssumption::OwnVirtualCapital,
            OverheadAllocation::NotAllocated,
        )
        .unwrap();
        assert_eq!(report.transaction_net.unwrap().to_string(), "-1050");
    }
    #[test]
    fn missing_is_not_zero_and_duplicate_components_cannot_double_count() {
        let quote = Quote {
            starting_asset: base_asset(),
            amount_in: 100.into(),
            amount_out: 110.into(),
            includes_pool_fees_and_impact: true,
            quote_reference: "synthetic".into(),
        };
        let known = fee(
            NetworkId::BaseMainnet,
            ExpenseKind::NetworkExecution,
            0,
            1,
            1,
        );
        let absent = evaluate_costs(
            &quote,
            std::slice::from_ref(&known),
            FundingAssumption::OwnVirtualCapital,
            OverheadAllocation::NotAllocated,
        )
        .unwrap();
        assert!(absent.transaction_net.is_none());
        assert!(!absent.costs_complete());
        let zero = fee(NetworkId::BaseMainnet, ExpenseKind::BaseL1Data, 0, 1, 1);
        let complete = evaluate_costs(
            &quote,
            &[known.clone(), zero.clone()],
            FundingAssumption::OwnVirtualCapital,
            OverheadAllocation::NotAllocated,
        )
        .unwrap();
        assert_eq!(complete.transaction_net.unwrap().to_string(), "10");
        let unknown = evaluate_costs(
            &quote,
            &[known.clone(), zero.clone()],
            FundingAssumption::Unknown {
                reason: "funding not modeled".into(),
            },
            OverheadAllocation::NotAllocated,
        )
        .unwrap();
        assert!(unknown.transaction_net.is_none());
        assert!(
            evaluate_costs(
                &quote,
                &[known.clone(), known, zero],
                FundingAssumption::OwnVirtualCapital,
                OverheadAllocation::NotAllocated
            )
            .is_err()
        );
        let absent = Expense {
            kind: ExpenseKind::NetworkExecution,
            amount: ExpenseAmount::Missing {
                reason: "RPC fee unavailable".into(),
            },
        };
        assert!(
            evaluate_costs(
                &quote,
                &[absent],
                FundingAssumption::OwnVirtualCapital,
                OverheadAllocation::NotAllocated
            )
            .unwrap()
            .transaction_net
            .is_none()
        );
    }
    #[test]
    fn same_asset_costs_need_no_fx_and_ratio_costs_round_up() {
        let asset = base_asset();
        let quote = Quote {
            starting_asset: asset.clone(),
            amount_in: 100.into(),
            amount_out: 110.into(),
            includes_pool_fees_and_impact: true,
            quote_reference: "synthetic".into(),
        };
        let direct = Expense {
            kind: ExpenseKind::Other,
            amount: ExpenseAmount::Known {
                asset: AccountingAsset::Token(asset),
                amount: 2.into(),
                valuation: Valuation::SameAsset {
                    reference: "same-token-units".into(),
                    valued_at_unix_ms: 1,
                },
            },
        };
        let expenses = [
            fee(
                NetworkId::BaseMainnet,
                ExpenseKind::NetworkExecution,
                1,
                1,
                3,
            ),
            fee(NetworkId::BaseMainnet, ExpenseKind::BaseL1Data, 0, 1, 1),
            direct,
        ];
        let report = evaluate_costs(
            &quote,
            &expenses,
            FundingAssumption::OwnVirtualCapital,
            OverheadAllocation::NotAllocated,
        )
        .unwrap();
        assert_eq!(report.transaction_net.unwrap().to_string(), "7");
        let bad = Quote {
            includes_pool_fees_and_impact: false,
            ..quote
        };
        assert!(
            evaluate_costs(
                &bad,
                &[],
                FundingAssumption::OwnVirtualCapital,
                OverheadAllocation::NotAllocated
            )
            .is_err()
        );
    }
}
