use crate::{AccountingAsset, PaperError, paper_error};
use arb_domain::{AssetId, AtomicAmount, NetworkId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LedgerAccount {
    Available,
    Reserved,
    InitialCapital,
    Market,
    Fees,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EntrySide {
    Debit,
    Credit,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Posting {
    pub asset: AccountingAsset,
    pub account: LedgerAccount,
    pub side: EntrySide,
    pub amount: AtomicAmount,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitialBalance {
    pub asset: AccountingAsset,
    pub amount: AtomicAmount,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReservationRequest {
    pub attempt_id: String,
    pub principal_asset: AssetId,
    pub principal: AtomicAmount,
    pub native_fee_budget: AtomicAmount,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum PaperOutcome {
    /// Entire atomic route succeeded; output is in the original principal asset.
    Succeeded {
        amount_out: AtomicAmount,
        actual_native_fee: AtomicAmount,
    },
    /// Atomic route reverted; principal returns, included fee is still spent.
    FailedIncluded { actual_native_fee: AtomicAmount },
    /// Positive scenario evidence says no inclusion/fee spend occurred.
    NotIncluded { reason: String },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum PaperCommand {
    Initialize {
        balances: Vec<InitialBalance>,
    },
    Reserve {
        request: ReservationRequest,
    },
    MarkUnknown {
        attempt_id: String,
        reason: String,
    },
    Resolve {
        attempt_id: String,
        outcome: PaperOutcome,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalEvent {
    run_id: String,
    network: NetworkId,
    #[serde(with = "sequence_wire")]
    sequence: u64,
    command_id: String,
    command: PaperCommand,
    postings: Vec<Posting>,
}
mod sequence_wire {
    use serde::{Deserialize, Deserializer, Serializer, de};
    pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        let value = String::deserialize(deserializer)?;
        if value.is_empty()
            || (value.len() > 1 && value.starts_with('0'))
            || !value.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(de::Error::custom("canonical sequence string required"));
        }
        value.parse().map_err(de::Error::custom)
    }
}
impl JournalEvent {
    pub fn run_id(&self) -> &str {
        &self.run_id
    }
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn command_id(&self) -> &str {
        &self.command_id
    }
    pub fn command(&self) -> &PaperCommand {
        &self.command
    }
    pub fn network(&self) -> NetworkId {
        self.network
    }
    pub fn postings(&self) -> &[Posting] {
        &self.postings
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Balance {
    pub free: AtomicAmount,
    pub reserved: AtomicAmount,
    pub total: AtomicAmount,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReservationState {
    Reserved,
    Unknown,
    Resolved,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Reservation {
    request: ReservationRequest,
    state: ReservationState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PortfolioBalance {
    pub asset: AccountingAsset,
    pub free: AtomicAmount,
    pub reserved: AtomicAmount,
    pub total: AtomicAmount,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PortfolioReservation {
    pub attempt_id: String,
    pub principal_asset: AssetId,
    pub principal: AtomicAmount,
    pub native_fee_budget: AtomicAmount,
    pub state: ReservationState,
}

/// Single-writer deterministic portfolio reducer. `&mut self` serializes local
/// commands; shared callers must use one Mutex/actor. Cross-process durability and
/// fencing require the storage integration and are deliberately not claimed here.
#[derive(Clone, Debug)]
pub struct PaperRun {
    run_id: String,
    network: NetworkId,
    free: HashMap<AccountingAsset, AtomicAmount>,
    reserved: HashMap<AccountingAsset, AtomicAmount>,
    attempts: HashMap<String, Reservation>,
    events: Vec<JournalEvent>,
    idempotency: HashMap<String, usize>,
}
fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-:.".contains(&b))
}
impl PaperRun {
    pub fn new(
        run_id: &str,
        network: NetworkId,
        balances: Vec<InitialBalance>,
    ) -> Result<Self, PaperError> {
        if !valid_id(run_id) {
            return Err(paper_error(
                "run_id",
                "run identifier must be 1..128 safe ASCII characters",
            ));
        }
        let mut run = Self::empty(run_id, network);
        run.apply("initialize", PaperCommand::Initialize { balances })?;
        Ok(run)
    }
    fn empty(run_id: &str, network: NetworkId) -> Self {
        Self {
            run_id: run_id.into(),
            network,
            free: HashMap::new(),
            reserved: HashMap::new(),
            attempts: HashMap::new(),
            events: Vec::new(),
            idempotency: HashMap::new(),
        }
    }
    pub fn run_id(&self) -> &str {
        &self.run_id
    }
    pub fn journal(&self) -> &[JournalEvent] {
        &self.events
    }
    pub fn balance(&self, asset: &AccountingAsset) -> Result<Balance, PaperError> {
        let free = self
            .free
            .get(asset)
            .cloned()
            .unwrap_or_else(AtomicAmount::zero);
        let reserved = self
            .reserved
            .get(asset)
            .cloned()
            .unwrap_or_else(AtomicAmount::zero);
        let total = free
            .checked_add(&reserved)
            .map_err(|_| paper_error("balance", "total balance exceeds supported range"))?;
        Ok(Balance {
            free,
            reserved,
            total,
        })
    }
    pub fn balances(&self) -> Result<Vec<PortfolioBalance>, PaperError> {
        let mut assets: Vec<AccountingAsset> = self
            .free
            .keys()
            .chain(self.reserved.keys())
            .cloned()
            .collect();
        assets.sort_by_key(|asset| match asset {
            AccountingAsset::Token(id) => format!("TOKEN:{id}"),
            AccountingAsset::Native(network) => format!("NATIVE:{network}"),
        });
        assets.dedup();
        assets
            .into_iter()
            .map(|asset| {
                let balance = self.balance(&asset)?;
                Ok(PortfolioBalance {
                    asset,
                    free: balance.free,
                    reserved: balance.reserved,
                    total: balance.total,
                })
            })
            .collect()
    }
    pub fn outstanding_reservations(&self) -> usize {
        self.attempts
            .values()
            .filter(|reservation| reservation.state != ReservationState::Resolved)
            .count()
    }
    pub fn reservations(&self) -> Vec<PortfolioReservation> {
        let mut result: Vec<_> = self
            .attempts
            .values()
            .map(|r| PortfolioReservation {
                attempt_id: r.request.attempt_id.clone(),
                principal_asset: r.request.principal_asset.clone(),
                principal: r.request.principal.clone(),
                native_fee_budget: r.request.native_fee_budget.clone(),
                state: r.state.clone(),
            })
            .collect();
        result.sort_by(|a, b| a.attempt_id.cmp(&b.attempt_id));
        result
    }
    /// Identical idempotency key+command returns its original event; changed body
    /// conflicts. A failed command cannot partially mutate balances or history.
    pub fn apply(
        &mut self,
        command_id: &str,
        command: PaperCommand,
    ) -> Result<JournalEvent, PaperError> {
        if !valid_id(command_id) {
            return Err(paper_error(
                "command_id",
                "command identifier must be 1..128 safe ASCII characters",
            ));
        }
        if let Some(index) = self.idempotency.get(command_id) {
            let event = &self.events[*index];
            return if event.command == command {
                Ok(event.clone())
            } else {
                Err(paper_error(
                    "command_id",
                    "idempotency key was already used with a different command",
                ))
            };
        }
        let mut next = self.clone();
        let event = next.apply_new_in_place(command_id, command)?;
        *self = next;
        Ok(event)
    }
    /// Apply a new command to disposable owned state. Callers validate the command
    /// identity and discard this entire state if any transition fails. Public
    /// apply uses a clone; replay owns a fresh run and never clones its history.
    fn apply_new_in_place(
        &mut self,
        command_id: &str,
        command: PaperCommand,
    ) -> Result<JournalEvent, PaperError> {
        let mut postings = Vec::new();
        match &command {
            PaperCommand::Initialize { balances } => {
                if !self.events.is_empty() {
                    return Err(paper_error(
                        "balances",
                        "reset requires a new paper run; initial balances are immutable",
                    ));
                }
                if balances.is_empty() {
                    return Err(paper_error(
                        "balances",
                        "initial virtual balances must be explicit",
                    ));
                }
                for balance in balances {
                    if balance.asset.network() != self.network
                        || self.free.contains_key(&balance.asset)
                    {
                        return Err(paper_error(
                            "balances",
                            "initial assets must be unique and belong to the run network",
                        ));
                    }
                    self.free
                        .insert(balance.asset.clone(), balance.amount.clone());
                    transfer(
                        &mut postings,
                        &balance.asset,
                        LedgerAccount::InitialCapital,
                        LedgerAccount::Available,
                        &balance.amount,
                    );
                }
            }
            PaperCommand::Reserve { request } => {
                if !valid_id(&request.attempt_id)
                    || request.principal_asset.network() != self.network
                    || request.principal.is_zero()
                {
                    return Err(paper_error(
                        "reservation",
                        "positive same-network principal and valid attempt ID are required",
                    ));
                }
                if self.attempts.contains_key(&request.attempt_id) {
                    return Err(paper_error(
                        "attempt_id",
                        "an attempt already exists; reuse the original command ID for an idempotent retry",
                    ));
                }
                let principal = AccountingAsset::Token(request.principal_asset.clone());
                let fee = AccountingAsset::Native(self.network);
                self.move_available_to_reserved(&principal, &request.principal, &mut postings)?;
                self.move_available_to_reserved(&fee, &request.native_fee_budget, &mut postings)?;
                self.attempts.insert(
                    request.attempt_id.clone(),
                    Reservation {
                        request: request.clone(),
                        state: ReservationState::Reserved,
                    },
                );
            }
            PaperCommand::MarkUnknown { attempt_id, reason } => {
                if reason.trim().is_empty() {
                    return Err(paper_error("reason", "unknown outcome requires a reason"));
                }
                let reservation = self
                    .attempts
                    .get_mut(attempt_id)
                    .ok_or_else(|| paper_error("attempt_id", "reservation does not exist"))?;
                if reservation.state == ReservationState::Resolved {
                    return Err(paper_error(
                        "attempt_id",
                        "resolved attempts cannot become unknown",
                    ));
                }
                reservation.state = ReservationState::Unknown;
                // Uncertain inclusion never releases capital and never becomes a failed fill.
            }
            PaperCommand::Resolve {
                attempt_id,
                outcome,
            } => {
                let reservation = self
                    .attempts
                    .get(attempt_id)
                    .cloned()
                    .ok_or_else(|| paper_error("attempt_id", "reservation does not exist"))?;
                if reservation.state == ReservationState::Resolved {
                    return Err(paper_error(
                        "attempt_id",
                        "attempt is already resolved; use original command ID for retry",
                    ));
                }
                let request = &reservation.request;
                let principal = AccountingAsset::Token(request.principal_asset.clone());
                let fee = AccountingAsset::Native(self.network);
                let actual_fee = match outcome {
                    PaperOutcome::Succeeded {
                        actual_native_fee, ..
                    }
                    | PaperOutcome::FailedIncluded { actual_native_fee } => {
                        actual_native_fee.clone()
                    }
                    PaperOutcome::NotIncluded { reason } => {
                        if reason.trim().is_empty() {
                            return Err(paper_error(
                                "outcome",
                                "no-inclusion resolution requires positive scenario evidence",
                            ));
                        }
                        AtomicAmount::zero()
                    }
                };
                if actual_fee > request.native_fee_budget {
                    return Err(paper_error(
                        "outcome.actual_native_fee",
                        "actual fee exceeds reservation; scenario cannot be settled within this budget",
                    ));
                }
                self.decrease_reserved(&principal, &request.principal)?;
                match outcome {
                    PaperOutcome::Succeeded { amount_out, .. } => {
                        transfer(
                            &mut postings,
                            &principal,
                            LedgerAccount::Reserved,
                            LedgerAccount::Market,
                            &request.principal,
                        );
                        self.increase_free(&principal, amount_out)?;
                        transfer(
                            &mut postings,
                            &principal,
                            LedgerAccount::Market,
                            LedgerAccount::Available,
                            amount_out,
                        );
                    }
                    PaperOutcome::FailedIncluded { .. } | PaperOutcome::NotIncluded { .. } => {
                        self.increase_free(&principal, &request.principal)?;
                        transfer(
                            &mut postings,
                            &principal,
                            LedgerAccount::Reserved,
                            LedgerAccount::Available,
                            &request.principal,
                        );
                    }
                }
                self.decrease_reserved(&fee, &request.native_fee_budget)?;
                let released_fee = request
                    .native_fee_budget
                    .checked_sub(&actual_fee)
                    .map_err(|_| paper_error("fees", "invalid fee reservation"))?;
                self.increase_free(&fee, &released_fee)?;
                transfer(
                    &mut postings,
                    &fee,
                    LedgerAccount::Reserved,
                    LedgerAccount::Available,
                    &released_fee,
                );
                transfer(
                    &mut postings,
                    &fee,
                    LedgerAccount::Reserved,
                    LedgerAccount::Fees,
                    &actual_fee,
                );
                self.attempts
                    .get_mut(attempt_id)
                    .expect("existing reservation")
                    .state = ReservationState::Resolved;
            }
        }
        if self.events.is_empty() && !matches!(command, PaperCommand::Initialize { .. }) {
            return Err(paper_error("run", "initialization event required first"));
        }
        for asset in self.free.keys().chain(self.reserved.keys()) {
            self.balance(asset)?;
        }
        let sequence = u64::try_from(self.events.len())
            .map_err(|_| paper_error("journal", "sequence overflow"))?;
        let event = JournalEvent {
            run_id: self.run_id.clone(),
            network: self.network,
            sequence,
            command_id: command_id.into(),
            command,
            postings,
        };
        self.idempotency
            .insert(command_id.into(), self.events.len());
        self.events.push(event.clone());
        Ok(event)
    }
    fn move_available_to_reserved(
        &mut self,
        asset: &AccountingAsset,
        amount: &AtomicAmount,
        postings: &mut Vec<Posting>,
    ) -> Result<(), PaperError> {
        let free = self
            .free
            .get(asset)
            .cloned()
            .unwrap_or_else(AtomicAmount::zero);
        let remaining = free.checked_sub(amount).map_err(|_| {
            paper_error(
                "reservation",
                "insufficient free principal or native fee inventory",
            )
        })?;
        let reserved = self
            .reserved
            .get(asset)
            .cloned()
            .unwrap_or_else(AtomicAmount::zero)
            .checked_add(amount)
            .map_err(|_| paper_error("reservation", "reserved balance overflow"))?;
        self.free.insert(asset.clone(), remaining);
        self.reserved.insert(asset.clone(), reserved);
        transfer(
            postings,
            asset,
            LedgerAccount::Available,
            LedgerAccount::Reserved,
            amount,
        );
        Ok(())
    }
    fn increase_free(
        &mut self,
        asset: &AccountingAsset,
        amount: &AtomicAmount,
    ) -> Result<(), PaperError> {
        let balance = self
            .free
            .get(asset)
            .cloned()
            .unwrap_or_else(AtomicAmount::zero)
            .checked_add(amount)
            .map_err(|_| paper_error("balance", "free balance overflow"))?;
        self.free.insert(asset.clone(), balance);
        Ok(())
    }
    fn decrease_reserved(
        &mut self,
        asset: &AccountingAsset,
        amount: &AtomicAmount,
    ) -> Result<(), PaperError> {
        let balance = self
            .reserved
            .get(asset)
            .cloned()
            .unwrap_or_else(AtomicAmount::zero)
            .checked_sub(amount)
            .map_err(|_| paper_error("reservation", "reserved balance underflow"))?;
        self.reserved.insert(asset.clone(), balance);
        Ok(())
    }
    /// Import only a complete ordered journal. Recompute every posting from the
    /// command and compare; gaps, changed postings, run mixing and duplicate events
    /// fail. Replay mutates only a fresh owned run, so history copying is linear
    /// in journal size; any failure discards that run. Public command application
    /// retains its separate atomic clone-and-commit behavior. Persisted prefix
    /// replay models recovery, not actual disk durability.
    pub fn replay(events: &[JournalEvent]) -> Result<Self, PaperError> {
        let first = events
            .first()
            .ok_or_else(|| paper_error("journal", "empty journal cannot restore a run"))?;
        if !valid_id(&first.run_id) {
            return Err(paper_error("journal", "invalid run identity"));
        }
        let mut run = Self::empty(&first.run_id, first.network);
        for expected in events {
            if expected.run_id != run.run_id
                || expected.network != run.network
                || expected.sequence != run.events.len() as u64
            {
                return Err(paper_error(
                    "journal",
                    "journal run/network/sequence mismatch",
                ));
            }
            if !valid_id(&expected.command_id) || run.idempotency.contains_key(&expected.command_id)
            {
                return Err(paper_error(
                    "journal",
                    "journal command identity is invalid or duplicated",
                ));
            }
            // A failed transition or comparison discards this fresh owned run.
            // Replaying N entries copies each entry a constant number of times,
            // rather than cloning all prior history for every transition.
            let actual = run.apply_new_in_place(&expected.command_id, expected.command.clone())?;
            if actual != *expected {
                return Err(paper_error(
                    "journal",
                    "postings differ from deterministic command replay",
                ));
            }
        }
        Ok(run)
    }
}
fn transfer(
    postings: &mut Vec<Posting>,
    asset: &AccountingAsset,
    from: LedgerAccount,
    to: LedgerAccount,
    amount: &AtomicAmount,
) {
    if amount.is_zero() {
        return;
    }
    postings.push(Posting {
        asset: asset.clone(),
        account: from,
        side: EntrySide::Credit,
        amount: amount.clone(),
    });
    postings.push(Posting {
        asset: asset.clone(),
        account: to,
        side: EntrySide::Debit,
        amount: amount.clone(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier, Mutex};
    fn token() -> AssetId {
        AssetId::new(
            NetworkId::BaseMainnet,
            "0x0000000000000000000000000000000000000001",
        )
        .unwrap()
    }
    fn principal_asset() -> AccountingAsset {
        AccountingAsset::Token(token())
    }
    fn fee_asset() -> AccountingAsset {
        AccountingAsset::Native(NetworkId::BaseMainnet)
    }
    fn run() -> PaperRun {
        PaperRun::new(
            "paper-run-1",
            NetworkId::BaseMainnet,
            vec![
                InitialBalance {
                    asset: principal_asset(),
                    amount: 100.into(),
                },
                InitialBalance {
                    asset: fee_asset(),
                    amount: 10.into(),
                },
            ],
        )
        .unwrap()
    }
    fn reserve(id: &str, principal: u64, fee: u64) -> PaperCommand {
        PaperCommand::Reserve {
            request: ReservationRequest {
                attempt_id: id.into(),
                principal_asset: token(),
                principal: principal.into(),
                native_fee_budget: fee.into(),
            },
        }
    }
    #[test]
    fn concurrent_candidates_cannot_double_reserve_the_same_inventory() {
        let shared = Arc::new(Mutex::new(run()));
        let barrier = Arc::new(Barrier::new(3));
        let mut workers = Vec::new();
        for index in 0..2 {
            let shared = Arc::clone(&shared);
            let barrier = Arc::clone(&barrier);
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                shared
                    .lock()
                    .unwrap()
                    .apply(
                        &format!("cmd-{index}"),
                        reserve(&format!("attempt-{index}"), 60, 8),
                    )
                    .is_ok()
            }));
        }
        barrier.wait();
        let successes = workers
            .into_iter()
            .map(|worker| u8::from(worker.join().unwrap()))
            .sum::<u8>();
        assert_eq!(successes, 1);
        let run = shared.lock().unwrap();
        assert_eq!(
            run.balance(&principal_asset()).unwrap(),
            Balance {
                free: 40.into(),
                reserved: 60.into(),
                total: 100.into()
            }
        );
        assert_eq!(run.balance(&fee_asset()).unwrap().free, 2.into());
    }
    #[test]
    fn fees_cannot_fund_principal_and_failed_reservation_is_atomic() {
        let mut fees_only = PaperRun::new(
            "fee-only",
            NetworkId::BaseMainnet,
            vec![InitialBalance {
                asset: fee_asset(),
                amount: 10000.into(),
            }],
        )
        .unwrap();
        assert!(
            fees_only
                .apply("reserve", reserve("attempt", 1, 1))
                .is_err()
        );
        assert_eq!(fees_only.journal().len(), 1);
        let mut run = run();
        let before = run.journal().to_vec();
        assert!(run.apply("reserve", reserve("attempt", 50, 11)).is_err());
        assert_eq!(run.journal(), before);
        assert_eq!(run.balance(&principal_asset()).unwrap().free, 100.into());
    }
    #[test]
    fn outcome_scenarios_release_or_spend_exact_reservations() {
        for (outcome, expected_principal, expected_fee) in [
            (
                PaperOutcome::Succeeded {
                    amount_out: 55.into(),
                    actual_native_fee: 3.into(),
                },
                95,
                7,
            ),
            (
                PaperOutcome::Succeeded {
                    amount_out: 70.into(),
                    actual_native_fee: 3.into(),
                },
                110,
                7,
            ),
            (
                PaperOutcome::FailedIncluded {
                    actual_native_fee: 4.into(),
                },
                100,
                6,
            ),
            (
                PaperOutcome::NotIncluded {
                    reason: "modeled expiry before inclusion".into(),
                },
                100,
                10,
            ),
        ] {
            let mut run = run();
            run.apply("reserve", reserve("a", 60, 8)).unwrap();
            run.apply(
                "resolve",
                PaperCommand::Resolve {
                    attempt_id: "a".into(),
                    outcome,
                },
            )
            .unwrap();
            assert_eq!(
                run.balance(&principal_asset()).unwrap(),
                Balance {
                    free: expected_principal.into(),
                    reserved: 0.into(),
                    total: expected_principal.into()
                }
            );
            assert_eq!(run.balance(&fee_asset()).unwrap().free, expected_fee.into());
            assert_eq!(run.balance(&fee_asset()).unwrap().reserved, 0.into());
            assert_pairwise_conservation(run.journal());
        }
    }
    #[test]
    fn unknown_outcomes_hold_funds_and_budget_overrun_never_partially_settles() {
        let mut run = run();
        run.apply("reserve", reserve("a", 60, 8)).unwrap();
        run.apply(
            "unknown",
            PaperCommand::MarkUnknown {
                attempt_id: "a".into(),
                reason: "modeled response timeout".into(),
            },
        )
        .unwrap();
        let before = run.journal().to_vec();
        assert_eq!(run.balance(&principal_asset()).unwrap().reserved, 60.into());
        assert!(
            run.apply(
                "resolve",
                PaperCommand::Resolve {
                    attempt_id: "a".into(),
                    outcome: PaperOutcome::FailedIncluded {
                        actual_native_fee: 9.into()
                    }
                }
            )
            .is_err()
        );
        assert_eq!(run.journal(), before);
        assert_eq!(run.balance(&fee_asset()).unwrap().reserved, 8.into());
    }
    #[test]
    fn journal_prefix_recovery_and_retry_are_idempotent() {
        let mut run = run();
        let command = reserve("a", 60, 8);
        let event = run.apply("reserve", command.clone()).unwrap();
        assert_eq!(run.apply("reserve", command.clone()).unwrap(), event);
        assert_eq!(run.journal().len(), 2);
        assert!(run.apply("reserve", reserve("a", 61, 8)).is_err());
        let encoded = serde_json::to_string(run.journal()).unwrap();
        let decoded: Vec<JournalEvent> = serde_json::from_str(&encoded).unwrap();
        let mut restored = PaperRun::replay(&decoded).unwrap();
        assert_eq!(restored.apply("reserve", command).unwrap(), event);
        assert_eq!(restored.journal().len(), 2);
        restored
            .apply(
                "resolve",
                PaperCommand::Resolve {
                    attempt_id: "a".into(),
                    outcome: PaperOutcome::FailedIncluded {
                        actual_native_fee: 3.into(),
                    },
                },
            )
            .unwrap();
        for length in 1..=restored.journal().len() {
            let prefix = PaperRun::replay(&restored.journal()[..length]).unwrap();
            assert_eq!(prefix.journal(), &restored.journal()[..length]);
            assert_pairwise_conservation(prefix.journal());
        }
        let rebuilt = PaperRun::replay(restored.journal()).unwrap();
        assert_eq!(
            rebuilt.balance(&principal_asset()).unwrap(),
            restored.balance(&principal_asset()).unwrap()
        );
        assert_eq!(
            rebuilt.balance(&fee_asset()).unwrap(),
            restored.balance(&fee_asset()).unwrap()
        );
    }
    #[test]
    fn large_journal_replay_retains_unknown_funds_and_rejects_late_corruption() {
        // Build a three-command cycle using the public atomic reducer, then
        // independently repeat its known postings with unique journal identities.
        // Fixture construction therefore does not replay the new private path.
        let mut template = run();
        template.apply("reserve", reserve("attempt", 1, 1)).unwrap();
        template
            .apply(
                "unknown",
                PaperCommand::MarkUnknown {
                    attempt_id: "attempt".into(),
                    reason: "synthetic unknown inclusion".into(),
                },
            )
            .unwrap();
        template
            .apply(
                "release",
                PaperCommand::Resolve {
                    attempt_id: "attempt".into(),
                    outcome: PaperOutcome::NotIncluded {
                        reason: "synthetic terminal no-inclusion evidence".into(),
                    },
                },
            )
            .unwrap();
        let mut events = vec![template.journal()[0].clone()];
        for cycle in 0..1666 {
            for original in &template.journal()[1..] {
                let mut event = original.clone();
                event.sequence = events.len() as u64;
                event.command_id = format!("{}-{cycle}", original.command_id);
                let id = format!("attempt-{cycle}");
                match &mut event.command {
                    PaperCommand::Reserve { request } => request.attempt_id = id,
                    PaperCommand::MarkUnknown { attempt_id, .. }
                    | PaperCommand::Resolve { attempt_id, .. } => *attempt_id = id,
                    PaperCommand::Initialize { .. } => panic!("cycle cannot initialize"),
                }
                events.push(event);
            }
        }
        let terminal = events.pop().unwrap();
        assert_eq!(events.len(), 4998);
        let mut restored = PaperRun::replay(&events).unwrap();
        assert_eq!(restored.journal(), events);
        assert_eq!(restored.outstanding_reservations(), 1);
        assert_eq!(
            restored.balance(&principal_asset()).unwrap().free,
            99.into()
        );
        assert_eq!(
            restored.balance(&principal_asset()).unwrap().reserved,
            1.into()
        );
        assert_eq!(restored.balance(&fee_asset()).unwrap().free, 9.into());
        assert_eq!(restored.balance(&fee_asset()).unwrap().reserved, 1.into());
        assert_eq!(
            restored
                .reservations()
                .iter()
                .find(|r| r.attempt_id == "attempt-1665")
                .unwrap()
                .state,
            ReservationState::Unknown
        );
        // Old idempotency receipts survive the complete replay unchanged.
        let retry = &events[2500];
        assert_eq!(
            restored
                .apply(&retry.command_id, retry.command.clone())
                .unwrap(),
            *retry
        );
        assert_eq!(restored.journal().len(), 4998);
        assert_eq!(
            restored
                .apply(&terminal.command_id, terminal.command.clone())
                .unwrap(),
            terminal
        );
        assert_eq!(restored.outstanding_reservations(), 0);
        assert_eq!(
            restored.balance(&principal_asset()).unwrap().free,
            100.into()
        );
        assert_eq!(restored.balance(&fee_asset()).unwrap().free, 10.into());
        assert_pairwise_conservation(restored.journal());
        // Corruption near the journal end must still discard the entire replay.
        let mut corrupted = restored.journal().to_vec();
        corrupted.last_mut().unwrap().postings[0].amount = 2.into();
        assert!(PaperRun::replay(&corrupted).is_err());
        let mut duplicated = restored.journal().to_vec();
        duplicated.last_mut().unwrap().command_id = duplicated[1].command_id.clone();
        assert!(PaperRun::replay(&duplicated).is_err());
    }
    #[test]
    fn replay_rejects_tampering_gaps_and_cross_run_history() {
        let mut run = run();
        run.apply("reserve", reserve("a", 60, 8)).unwrap();
        let mut tampered = run.journal().to_vec();
        tampered[1].postings[0].amount = 59.into();
        assert!(PaperRun::replay(&tampered).is_err());
        let mut gap = run.journal().to_vec();
        gap[1].sequence = 2;
        assert!(PaperRun::replay(&gap).is_err());
        let mut mixed = run.journal().to_vec();
        mixed[1].run_id = "different".into();
        assert!(PaperRun::replay(&mixed).is_err());
        assert!(
            run.apply(
                "reset",
                PaperCommand::Initialize {
                    balances: vec![InitialBalance {
                        asset: principal_asset(),
                        amount: 999.into()
                    }]
                }
            )
            .is_err()
        );
    }
    #[test]
    fn bounded_reserve_release_properties_preserve_initial_balances() {
        for amount in 1..=100 {
            for fee in 0..=10 {
                let mut run = run();
                run.apply("reserve", reserve("a", amount, fee)).unwrap();
                run.apply(
                    "release",
                    PaperCommand::Resolve {
                        attempt_id: "a".into(),
                        outcome: PaperOutcome::NotIncluded {
                            reason: "known not included".into(),
                        },
                    },
                )
                .unwrap();
                assert_eq!(run.balance(&principal_asset()).unwrap().total, 100.into());
                assert_eq!(run.balance(&fee_asset()).unwrap().total, 10.into());
                assert_pairwise_conservation(run.journal());
            }
        }
    }
    fn assert_pairwise_conservation(events: &[JournalEvent]) {
        for event in events {
            assert_eq!(event.postings.len() % 2, 0);
            for pair in event.postings.chunks_exact(2) {
                assert_eq!(pair[0].asset, pair[1].asset);
                assert_eq!(pair[0].amount, pair[1].amount);
                assert_eq!(pair[0].side, EntrySide::Credit);
                assert_eq!(pair[1].side, EntrySide::Debit);
            }
        }
    }
}
