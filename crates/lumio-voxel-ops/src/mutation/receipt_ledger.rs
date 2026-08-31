//! Txn receipt ledger. Does not publish chunks and does not hold a directory root.

use super::fingerprint::{MUTATION_RECEIPT_SCHEMA, MutationRequest, canonical_fingerprint};
use super::reservation::MutationReservation;
use lumio_voxel_contracts::{SCHEMA_IDS, STABLE_ERROR_IDS};
use lumio_voxel_domain::config_snapshot::VoxelConfigSnapshot;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayDisposition {
    Original,
    Duplicate,
    Conflict,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LookupOutcome {
    Vacant,
    InFlight,
    Duplicate { receipt: Vec<u8> },
}

/// Public status projection used by the generated world Port. An absent entry
/// is `Unknown`; in-flight and finalized entries retain their protocol state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiptStatus {
    Unknown,
    Prepared,
    Applied,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FinalizeOutcome {
    pub disposition: ReplayDisposition,
    pub receipt: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LedgerError {
    error_id: &'static str,
    disposition: Option<ReplayDisposition>,
}

impl LedgerError {
    pub fn error_id(&self) -> &'static str {
        self.error_id
    }

    pub fn disposition(&self) -> Option<ReplayDisposition> {
        self.disposition
    }

    fn conflict() -> Self {
        Self {
            error_id: stable("RevisionConflict"),
            disposition: Some(ReplayDisposition::Conflict),
        }
    }

    fn budget() -> Self {
        Self {
            error_id: stable("BudgetExceeded"),
            disposition: None,
        }
    }

    fn invalid_handle() -> Self {
        Self {
            error_id: stable("InvalidHandle"),
            disposition: None,
        }
    }

    fn session_mismatch() -> Self {
        Self {
            error_id: stable("SessionMismatch"),
            disposition: None,
        }
    }
}

struct LedgerEntry {
    reservation: MutationReservation,
    receipt: Option<Vec<u8>>,
}

pub struct ReceiptLedger {
    snapshot: Arc<VoxelConfigSnapshot>,
    max_entries: usize,
    entries: BTreeMap<String, LedgerEntry>,
    reserve_count: usize,
    bound_world_id: Option<String>,
    bound_generation: Option<u64>,
}

impl ReceiptLedger {
    /// Bind a ledger to an approved snapshot. `max_entries` is adapter-internal;
    /// there is no unbounded constructor and no Schema capacity column.
    pub fn from_approved_snapshot(
        snapshot: Arc<VoxelConfigSnapshot>,
        max_entries: usize,
    ) -> Result<Self, LedgerError> {
        debug_assert!(SCHEMA_IDS.contains(&MUTATION_RECEIPT_SCHEMA));
        if max_entries == 0 {
            return Err(LedgerError::budget());
        }
        Ok(Self {
            snapshot,
            max_entries,
            entries: BTreeMap::new(),
            reserve_count: 0,
            bound_world_id: None,
            bound_generation: None,
        })
    }

    pub fn config_hash(&self) -> &str {
        self.snapshot.config_hash()
    }

    pub fn reserve_count(&self) -> usize {
        self.reserve_count
    }

    pub fn lookup(&self, request: &MutationRequest) -> Result<LookupOutcome, LedgerError> {
        self.check_request(request)?;
        let Some(entry) = self.entries.get(&request.txn_id) else {
            return Ok(LookupOutcome::Vacant);
        };
        Self::check_entry(request, entry)?;
        match &entry.receipt {
            Some(receipt) => Ok(LookupOutcome::Duplicate {
                receipt: receipt.clone(),
            }),
            None => Ok(LookupOutcome::InFlight),
        }
    }

    /// Look up a transaction without reconstructing its request fingerprint.
    /// The Port's `status(txnId)` method intentionally carries only the generated
    /// transaction identity; request validation remains on prepare/commit paths.
    pub fn status(&self, txn_id: &str) -> ReceiptStatus {
        let Some(entry) = self.entries.get(txn_id) else {
            return ReceiptStatus::Unknown;
        };
        if entry.receipt.is_some() {
            ReceiptStatus::Applied
        } else {
            ReceiptStatus::Prepared
        }
    }

    pub fn reserve(
        &mut self,
        request: &MutationRequest,
    ) -> Result<MutationReservation, LedgerError> {
        self.check_request(request)?;
        if let Some(entry) = self.entries.get(&request.txn_id) {
            Self::check_entry(request, entry)?;
            return Ok(entry.reservation.clone());
        }
        if self.entries.len() >= self.max_entries {
            return Err(LedgerError::budget());
        }
        let reservation = MutationReservation::from_request(request)
            .map_err(|_| LedgerError::invalid_handle())?;
        self.bind(request);
        self.entries.insert(
            request.txn_id.clone(),
            LedgerEntry {
                reservation: reservation.clone(),
                receipt: None,
            },
        );
        self.reserve_count = self.reserve_count.saturating_add(1);
        Ok(reservation)
    }

    pub fn finalize(
        &mut self,
        request: &MutationRequest,
        receipt: Vec<u8>,
    ) -> Result<FinalizeOutcome, LedgerError> {
        self.check_request(request)?;
        let Some(entry) = self.entries.get_mut(&request.txn_id) else {
            return Err(LedgerError::invalid_handle());
        };
        Self::check_entry(request, entry)?;
        match &entry.receipt {
            Some(stored) => Ok(FinalizeOutcome {
                disposition: ReplayDisposition::Duplicate,
                receipt: stored.clone(),
            }),
            None => {
                entry.receipt = Some(receipt.clone());
                Ok(FinalizeOutcome {
                    disposition: ReplayDisposition::Original,
                    receipt,
                })
            }
        }
    }

    /// Private in-flight release. Completed receipts are retained.
    pub fn abort(&mut self, request: &MutationRequest) -> Result<(), LedgerError> {
        self.check_request(request)?;
        let Some(entry) = self.entries.get(&request.txn_id) else {
            return Ok(());
        };
        Self::check_entry(request, entry)?;
        if entry.receipt.is_some() {
            return Ok(());
        }
        self.entries.remove(&request.txn_id);
        Ok(())
    }

    /// Drop every in-flight reservation. Completed receipts stay put.
    pub fn abort_in_flight(&mut self) {
        self.entries.retain(|_, entry| entry.receipt.is_some());
    }

    pub fn in_flight_count(&self) -> usize {
        self.entries
            .values()
            .filter(|entry| entry.receipt.is_none())
            .count()
    }

    fn check_request(&self, request: &MutationRequest) -> Result<(), LedgerError> {
        if request.txn_id.is_empty() || request.world_id.is_empty() {
            return Err(LedgerError::invalid_handle());
        }
        if let Some(world_id) = &self.bound_world_id
            && world_id != &request.world_id
        {
            return Err(LedgerError::session_mismatch());
        }
        if let Some(generation) = self.bound_generation
            && generation != request.generation
        {
            return Err(LedgerError::invalid_handle());
        }
        Ok(())
    }

    fn check_entry(request: &MutationRequest, entry: &LedgerEntry) -> Result<(), LedgerError> {
        if entry.reservation.world_id() != request.world_id {
            return Err(LedgerError::session_mismatch());
        }
        if entry.reservation.is_expired(request.generation) {
            return Err(LedgerError::invalid_handle());
        }
        let fingerprint =
            canonical_fingerprint(request).map_err(|_| LedgerError::invalid_handle())?;
        if fingerprint != entry.reservation.fingerprint() {
            return Err(LedgerError::conflict());
        }
        Ok(())
    }

    fn bind(&mut self, request: &MutationRequest) {
        if self.bound_world_id.is_none() {
            self.bound_world_id = Some(request.world_id.clone());
        }
        if self.bound_generation.is_none() {
            self.bound_generation = Some(request.generation);
        }
    }
}

fn stable(id: &'static str) -> &'static str {
    debug_assert!(STABLE_ERROR_IDS.contains(&id));
    id
}
