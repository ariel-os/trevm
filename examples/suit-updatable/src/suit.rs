use core::{cell::RefCell, net::SocketAddr, str::FromStr};

use alloc::vec::Vec;
use ariel_os::debug::log::{Debug2Format, error};
use ariel_os::time::Duration;
use dress_up::manifest::Manifest;
use dress_up::{AsyncOperatingHooks, Authenticated, SuitManifest};
use uuid::Uuid;

use cose_nostd::{
    iana::{Algorithm, EllipticCurve, KeyOperation, KeyType, key_labels},
    key::CoseKeyBuilder,
    signature::sign1::CoseSign1,
};

use crate::coap_fetch::{CoapFetchError, get_blockwise};

pub const MAX_CAPSULE_SIZE: usize = 100 * 1024;
const STAGING_SLOT: u64 = 1;

pub const PUBKEY_P256: &[u8; 65] = include_bytes!("../suit/demo-public-key-p256.bin");

pub fn suit_vendor_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_DNS, "example.com".as_bytes())
}

pub fn suit_class_id() -> Uuid {
    Uuid::new_v5(&suit_vendor_id(), "trevm-suit-updatable-demo".as_bytes())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuitPhase {
    ParseEnvelope,
    Authentication,
    PayloadFetch,
    PayloadInstallation,
    ImageValidation,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UpdateError {
    UnsupportedManifestVersion,
    EmptyCapsule,
    CapsuleTooLarge,
    InvalidSlot,
    OutOfBoundsRead,
    MalformedUri,
    CoapRequestFailed,
    CoapTimeout,
    AllocationFailed {
        size: usize,
    },

    VendorIdMismatch {
        expected: Uuid,
        actual: Uuid,
    },
    ClassIdMismatch {
        expected: Uuid,
        actual: Uuid,
    },
    ComponentSlotMismatch {
        expected: u64,
        actual: u64,
    },

    RollbackDetected {
        current: u64,
        attempted: u64,
    },
    UnsupportedWriteContent,

    SuitAuthenticationFailed,
    SuitMissingAuthentication,
    SuitMissingSequenceNumber,
    SuitPayloadFetchConditionFailed {
        position: usize,
    },
    SuitPayloadInstallationConditionFailed {
        position: usize,
    },
    SuitImageValidationConditionFailed {
        position: usize,
    },
    SuitConditionFailed {
        phase: SuitPhase,
        position: usize,
    },
    SuitExecutionFailed,

    DressUp {
        phase: SuitPhase,
        error: dress_up::error::Error,
    },
}

impl UpdateError {
    fn from_suit_error(phase: SuitPhase, e: dress_up::error::Error) -> Self {
        match e {
            dress_up::error::Error::UnsupportedManifestVersion => Self::UnsupportedManifestVersion,
            dress_up::error::Error::AuthenticationFailure => Self::SuitAuthenticationFailed,
            dress_up::error::Error::NoAuthObject => match phase {
                SuitPhase::Authentication => Self::SuitMissingAuthentication,
                _ => Self::SuitExecutionFailed,
            },
            dress_up::error::Error::NoSequenceNumber => Self::SuitMissingSequenceNumber,
            dress_up::error::Error::ConditionMatchFail { position } => match phase {
                SuitPhase::PayloadFetch => Self::SuitPayloadFetchConditionFailed { position },
                SuitPhase::PayloadInstallation => {
                    Self::SuitPayloadInstallationConditionFailed { position }
                }
                SuitPhase::ImageValidation => Self::SuitImageValidationConditionFailed { position },
                _ => Self::SuitConditionFailed { phase, position },
            },
            dress_up::error::Error::EndOfInput => Self::EmptyCapsule,
            _ => Self::DressUp { phase, error: e },
        }
    }
}

struct TrevmSuitHooks {
    staging: RefCell<Vec<u8>>,
    last_error: RefCell<Option<UpdateError>>,
}

impl TrevmSuitHooks {
    fn new() -> Self {
        Self {
            staging: RefCell::new(Vec::new()),
            last_error: RefCell::new(None),
        }
    }

    fn into_capsule(self) -> Result<Vec<u8>, UpdateError> {
        let capsule = self.staging.into_inner();
        if capsule.is_empty() {
            return Err(UpdateError::EmptyCapsule);
        }

        Ok(capsule)
    }

    fn remember_error(&self, error: UpdateError) -> dress_up::error::Error {
        *self.last_error.borrow_mut() = Some(error);

        match error {
            UpdateError::EmptyCapsule => dress_up::error::Error::EndOfInput,

            UpdateError::CapsuleTooLarge
            | UpdateError::InvalidSlot
            | UpdateError::OutOfBoundsRead => {
                dress_up::error::Error::ConditionMatchFail { position: 0 }
            }

            _ => dress_up::error::Error::ConditionMatchFail { position: 0 },
        }
    }

    fn remember_condition_mismatch(&self, error: UpdateError) {
        *self.last_error.borrow_mut() = Some(error);
    }

    fn take_last_error(&self) -> Option<UpdateError> {
        self.last_error.borrow_mut().take()
    }

    fn clear_last_error(&self) {
        *self.last_error.borrow_mut() = None;
    }

    fn map_phase_error(&self, phase: SuitPhase, err: dress_up::error::Error) -> UpdateError {
        match self.take_last_error() {
            Some(e) => e,
            None => UpdateError::from_suit_error(phase, err),
        }
    }
}

impl AsyncOperatingHooks for TrevmSuitHooks {
    type ReadWriteBufferSize = generic_array::typenum::U512;
    async fn match_vendor_id(
        &self,
        uuid: Uuid,
        _component: &dress_up::component::Component<'_>,
    ) -> Result<bool, dress_up::error::Error> {
        let ok = uuid == suit_vendor_id();
        if !ok {
            self.remember_condition_mismatch(UpdateError::VendorIdMismatch {
                expected: suit_vendor_id(),
                actual: uuid,
            });
        }
        Ok(ok)
    }

    async fn match_class_id(
        &self,
        uuid: Uuid,
        _component: &dress_up::component::Component<'_>,
    ) -> Result<bool, dress_up::error::Error> {
        let ok = uuid == suit_class_id();
        if !ok {
            self.remember_condition_mismatch(UpdateError::ClassIdMismatch {
                expected: suit_class_id(),
                actual: uuid,
            });
        }

        Ok(ok)
    }

    async fn match_component_slot(
        &self,
        _component: &dress_up::component::Component<'_>,
        slot: u64,
    ) -> Result<bool, dress_up::error::Error> {
        let ok = slot == STAGING_SLOT;
        if !ok {
            self.remember_condition_mismatch(UpdateError::ComponentSlotMismatch {
                expected: STAGING_SLOT,
                actual: slot,
            });
        }
        Ok(ok)
    }

    async fn component_capacity(
        &self,
        _component: &dress_up::component::Component<'_>,
    ) -> Result<usize, dress_up::error::Error> {
        Ok(MAX_CAPSULE_SIZE)
    }

    async fn component_size(
        &self,
        _component: &dress_up::component::Component<'_>,
    ) -> Result<usize, dress_up::error::Error> {
        Ok(self.staging.borrow().len())
    }

    async fn component_read(
        &self,
        _component: &dress_up::component::Component<'_>,
        slot: Option<u64>,
        offset: usize,
        bytes: &mut [u8],
    ) -> Result<(), dress_up::error::Error> {
        if slot.unwrap_or(STAGING_SLOT) != STAGING_SLOT {
            return Err(self.remember_error(UpdateError::InvalidSlot));
        }

        let staging = self.staging.borrow();
        let end = offset
            .checked_add(bytes.len())
            .ok_or_else(|| self.remember_error(UpdateError::OutOfBoundsRead))?;

        let src = staging
            .get(offset..end)
            .ok_or_else(|| self.remember_error(UpdateError::OutOfBoundsRead))?;

        bytes.copy_from_slice(src);
        Ok(())
    }

    async fn component_write(
        &self,
        _component: &dress_up::component::Component<'_>,
        _slot: Option<u64>,
        _offset: usize,
        _bytes: &[u8],
    ) -> Result<(), dress_up::error::Error> {
        *self.last_error.borrow_mut() = Some(UpdateError::UnsupportedWriteContent);

        Err(dress_up::error::Error::UnsupportedCommand {
            command: dress_up::consts::SuitCommand::WriteContent.into(),
        })
    }

    async fn fetch(
        &self,
        _component: &dress_up::component::Component<'_>,
        slot: Option<u64>,
        uri: &str,
    ) -> Result<(), dress_up::error::Error> {
        if slot.unwrap_or(STAGING_SLOT) != STAGING_SLOT {
            return Err(self.remember_error(UpdateError::InvalidSlot));
        }
        let path = uri
            .strip_prefix("coap://")
            .ok_or_else(|| self.remember_error(UpdateError::MalformedUri))?;

        let slash_idx = path
            .find('/')
            .ok_or_else(|| self.remember_error(UpdateError::MalformedUri))?;
        let (addr_str, path) = path.split_at(slash_idx);

        let addr = SocketAddr::from_str(addr_str)
            .map_err(|_| self.remember_error(UpdateError::MalformedUri))?;

        self.staging.borrow_mut().clear();

        let body = get_blockwise(addr, path, MAX_CAPSULE_SIZE, Duration::from_secs(1))
            .await
            .map_err(|e| self.remember_error(e.into()))?;

        *self.staging.borrow_mut() = body;
        Ok(())
    }
}

pub fn build_and_authenticate_manifest<'a>(
    envelope_bytes: &'a impl AsRef<[u8]>,
) -> Result<(Manifest<'a, Authenticated>, u64), UpdateError> {
    let suit = SuitManifest::from_bytes(envelope_bytes)
        .authenticate(verify_cose_signature)
        .map_err(|e| UpdateError::from_suit_error(SuitPhase::Authentication, e))?;

    let envelope = suit
        .envelope()
        .map_err(|e| UpdateError::from_suit_error(SuitPhase::ParseEnvelope, e))?;

    let manifest = envelope
        .manifest()
        .map_err(|e| UpdateError::from_suit_error(SuitPhase::ParseEnvelope, e))?;

    let version = manifest
        .version()
        .map_err(|e| UpdateError::from_suit_error(SuitPhase::ParseEnvelope, e))?;

    if version != 1 {
        return Err(UpdateError::UnsupportedManifestVersion);
    }

    let sequence_number = manifest
        .sequence_number()
        .map_err(|e| UpdateError::from_suit_error(SuitPhase::ParseEnvelope, e))?;

    Ok((manifest, sequence_number))
}

pub async fn fetch_and_verify_update(
    manifest: Manifest<'_, Authenticated>,
) -> Result<Vec<u8>, UpdateError> {
    let hooks = TrevmSuitHooks::new();

    if manifest
        .has_payload_fetch()
        .map_err(|e| UpdateError::from_suit_error(SuitPhase::PayloadFetch, e))?
    {
        manifest
            .async_execute_payload_fetch(&hooks)
            .await
            .map_err(|e| hooks.map_phase_error(SuitPhase::PayloadFetch, e))?;
    }
    hooks.clear_last_error();
    if manifest
        .has_payload_installation()
        .map_err(|e| UpdateError::from_suit_error(SuitPhase::PayloadInstallation, e))?
    {
        manifest
            .async_execute_payload_installation(&hooks)
            .await
            .map_err(|e| hooks.map_phase_error(SuitPhase::PayloadInstallation, e))?;
    }
    hooks.clear_last_error();

    if manifest
        .has_image_validation()
        .map_err(|e| UpdateError::from_suit_error(SuitPhase::ImageValidation, e))?
    {
        manifest
            .async_execute_image_validation(&hooks)
            .await
            .map_err(|e| hooks.map_phase_error(SuitPhase::PayloadFetch, e))?;
    }

    let capsule = hooks.into_capsule()?;

    Ok(capsule)
}

fn verify_cose_signature(
    cose_sign1: &[u8],
    detached_payload: &[u8],
) -> Result<bool, dress_up::error::Error> {
    // Expected SEC1 uncompressed form (as in the const above):
    // 0x04 || x[32] || y[32]
    if PUBKEY_P256.len() != 65 || PUBKEY_P256[0] != 0x04 {
        error!("P-256 public key is not uncompressed SEC1 format");
        return Err(dress_up::error::Error::AuthenticationFailure);
    }

    let x = &PUBKEY_P256[1..33];
    let y = &PUBKEY_P256[33..65];

    let mut key_buf = [0u8; 128];

    let verification_key = CoseKeyBuilder::new(key_buf.as_mut_slice(), 6)
        .and_then(|b| {
            b.add_generic_params(
                KeyType::EC2,
                None,
                Some(Algorithm::Es256),
                Some(&[KeyOperation::Verify]),
                None,
            )
        })
        .and_then(|b| b.add_param(key_labels::ec::CRV, EllipticCurve::P256))
        .and_then(|b| b.add_param_bytes(key_labels::ec::X, x))
        .and_then(|b| b.add_param_bytes(key_labels::ec::Y, y))
        .and_then(|b| b.build())
        .map_err(|e| {
            error!(
                "[SUIT] failed to build COSE verification key: {:?}",
                Debug2Format(&e)
            );
            dress_up::error::Error::AuthenticationFailure
        })?;

    let verifier = CoseSign1::from_slice(cose_sign1).map_err(|e| {
        error!("[SUIT] failed to decode COSE_Sign1: {:?}", Debug2Format(&e));
        dress_up::error::Error::AuthenticationFailure
    })?;

    match verifier.verify_detached(detached_payload, &verification_key, None, None) {
        Ok(_) => Ok(true),
        Err(e) => {
            error!(
                "[SUIT] COSE_Sign1 verification failed: {:?}",
                Debug2Format(&e)
            );
            Ok(false)
        }
    }
}

impl From<CoapFetchError> for UpdateError {
    fn from(value: CoapFetchError) -> Self {
        match value {
            CoapFetchError::Empty => UpdateError::EmptyCapsule,
            CoapFetchError::TooLarge => UpdateError::CapsuleTooLarge,
            CoapFetchError::RequestFailed => UpdateError::CoapRequestFailed,
            CoapFetchError::Timeout => UpdateError::CoapTimeout,
            CoapFetchError::AllocationFailed { size } => UpdateError::AllocationFailed { size },
        }
    }
}
