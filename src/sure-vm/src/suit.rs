use core::{cell::RefCell, net::SocketAddr, str::FromStr};

use alloc::vec::Vec;
use ariel_os_log::{Debug2Format, error};
use ariel_os_embassy::api::time::Duration;
use dress_up::manifest::Manifest;
use dress_up::{AsyncOperatingHooks, Authenticated, SuitManifest};
use uuid::Uuid;

use cose_nostd::{
    iana::{Algorithm, EllipticCurve, KeyOperation, KeyType, key_labels},
    key::CoseKeyBuilder,
    signature::sign1::CoseSign1,
};

pub const MAX_CAPSULE_SIZE: usize = 100 * 1024;
const STAGING_SLOT: u64 = 1;

pub fn suit_vendor_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_DNS, "example.com".as_bytes())
}

pub fn suit_class_id() -> Uuid {
    Uuid::new_v5(&suit_vendor_id(), "trevm-suit-updatable-demo".as_bytes())
}

struct TrevmSuitHooks<F> {
    staging: RefCell<Vec<u8>>,
    fetch_function: F
}

impl<F> TrevmSuitHooks<F> {
    fn new(fetch: F) -> Self {
        Self {
            staging: RefCell::new(Vec::new()),
            fetch_function: fetch
        }
    }

    fn into_capsule(self) -> Vec<u8> {
        self.staging.into_inner()
    }
}

impl<E, F> AsyncOperatingHooks for TrevmSuitHooks<F> where
    for<'a> F: AsyncFnOnce(SocketAddr, &'a str, usize, Duration) -> Result<Vec<u8>, E> + Copy
{
    type ReadWriteBufferSize = generic_array::typenum::U512;
    async fn match_vendor_id(
        &self,
        uuid: Uuid,
        _component: &dress_up::component::Component<'_>,
    ) -> Result<bool, dress_up::error::Error> {
        let ok = uuid == suit_vendor_id();
        Ok(ok)
    }

    async fn match_class_id(
        &self,
        uuid: Uuid,
        _component: &dress_up::component::Component<'_>,
    ) -> Result<bool, dress_up::error::Error> {

        let ok = uuid == suit_class_id();
        Ok(ok)
    }

    async fn match_component_slot(
        &self,
        _component: &dress_up::component::Component<'_>,
        slot: u64,
    ) -> Result<bool, dress_up::error::Error> {
        let ok = slot == STAGING_SLOT;
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
            return Err(dress_up::error::Error::ConditionMatchFail { position: 120 });
        }
        let staging = self.staging.borrow();

        let end = offset
            .checked_add(bytes.len())
            .ok_or(dress_up::error::Error::CapacityError)?;

        let src = staging
            .get(offset..end)
            .ok_or(dress_up::error::Error::CapacityError)?;

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
            return Err(dress_up::error::Error::ConditionMatchFail { position: 120 });
        }
        let path = uri
            .strip_prefix("coap://")
            .ok_or(dress_up::error::Error::Utf8Error { position: 0 })?;

        let slash_idx = path
            .find('/')
            .ok_or(dress_up::error::Error::Utf8Error { position: 0 })?;
        let (addr_str, path) = path.split_at(slash_idx);

        let addr = SocketAddr::from_str(addr_str)
            .map_err(|_e| dress_up::error::Error::Utf8Error { position: 0 })?;

        self.staging.borrow_mut().clear();

        let body = (self.fetch_function)(addr, path, MAX_CAPSULE_SIZE, Duration::from_secs(1)).await
            .map_err(|_e| dress_up::error::Error::InvalidCommandSequence { position: 24 })?;

        *self.staging.borrow_mut() = body;
        Ok(())
    }
}

pub fn build_and_authenticate_manifest<'a>(
    envelope_bytes: &'a impl AsRef<[u8]>,
    pub_key: &[u8]
) -> Result<(Manifest<'a, Authenticated>, u64), dress_up::error::Error> {
    let suit = SuitManifest::from_bytes(envelope_bytes)
        .authenticate(|cose, payload| verify_cose_signature(cose, payload, pub_key))?;

    let envelope = suit
        .envelope()?;

    let manifest = envelope
        .manifest()?;

    let version = manifest
        .version()?;

    if version != 1 {
        return Err(dress_up::error::Error::UnsupportedManifestVersion);
    }

    let sequence_number = manifest
        .sequence_number()?;

    Ok((manifest, sequence_number))
}

pub async fn fetch_and_verify_update<E>(
    manifest: Manifest<'_, Authenticated>,
    f: impl AsyncFn(SocketAddr, &str, usize, Duration) -> Result<Vec<u8>, E> + Copy
) -> Result<Vec<u8>, dress_up::error::Error> {
    let hooks = TrevmSuitHooks::new(f);

    if manifest
        .has_payload_fetch()?
    {
        manifest
            .async_execute_payload_fetch(&hooks).await?;
    }
    if manifest
        .has_payload_installation()?
    {
        manifest
            .async_execute_payload_installation(&hooks).await?;
    }

    if manifest
        .has_image_validation()?
    {
        manifest
            .async_execute_image_validation(&hooks).await?;
    }

    Ok(hooks.into_capsule())
}

fn verify_cose_signature(
    cose_sign1: &[u8],
    detached_payload: &[u8],
    pub_key: &[u8],
) -> Result<bool, dress_up::error::Error> {
    // Expected SEC1 uncompressed form (as in the const above):
    // 0x04 || x[32] || y[32]
    if pub_key.len() != 65 || pub_key[0] != 0x04 {
        error!("P-256 public key is not uncompressed SEC1 format");
        return Err(dress_up::error::Error::AuthenticationFailure);
    }

    let x = &pub_key[1..33];
    let y = &pub_key[33..65];

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
