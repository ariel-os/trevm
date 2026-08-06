use alloc::boxed::Box;
use alloc::vec::Vec;

use ariel_os::coap::coap_run;
use ariel_os::debug::log::{Debug2Format, info};

use coap_handler::Handler;
use coap_handler_implementations::{HandlerBuilder, ReportingHandlerBuilder, new_dispatcher};
use coap_message::{Code, OptionNumber};
use coap_message_utils::Error as CoapError;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;

static SUIT_VERIFY_SIGNAL: Signal<CriticalSectionRawMutex, Box<[u8]>> = Signal::new();

#[derive(Debug)]
pub enum VmEvent {
    Dropped,
    Finished,
}

struct VmControl {
    payload: Vec<u8>,
}

impl VmControl {
    fn new() -> Self {
        Self {
            payload: Vec::new(),
        }
    }
}

impl Handler for VmControl {
    type RequestData = (Option<u32>, u8);

    type ExtractRequestError = coap_message_utils::Error;
    type BuildResponseError<M: coap_message::MinimalWritableMessage> = coap_message_utils::Error;

    fn extract_request_data<M: coap_message::ReadableMessage>(
        &mut self,
        request: &M,
    ) -> Result<Self::RequestData, Self::ExtractRequestError> {
        use coap_message::MessageOption;
        use coap_message_utils::OptionsExt;

        match request.code().into() {
            coap_numbers::code::DELETE => {
                info!("Received DELETE request for SUIT-Manifest");
                request.options().ignore_elective_others()?;

                self.payload.clear();

                Ok((None, coap_numbers::code::DELETED))
            }

            coap_numbers::code::PUT => {
                info!("Received PUT request for program ");
                let mut block1: Option<u32> = None;

                request
                    .options()
                    .filter(|o| {
                        if o.number() == coap_numbers::option::BLOCK1
                            && let Some(n) = o.value_uint()
                            && block1.is_none()
                        {
                            block1 = Some(n);
                            false
                        } else {
                            true
                        }
                    })
                    .ignore_elective_others()?;

                // This is a bit of a simplification, but ignoring the block size and just
                // appending is really kind'a fine IMO.
                let block1_value = block1.unwrap_or(0);

                // FIXME there's probably a Size1 option; if so, reallocate to fail early.

                let szx = block1_value & 0x7;
                let blocksize = 1usize << (4 + szx);
                let offset = (block1_value >> 4) as usize * blocksize;

                if offset == 0 {
                    self.payload.clear();
                }
                if self.payload.len() != offset {
                    return Ok((None, coap_numbers::code::REQUEST_ENTITY_INCOMPLETE));
                }

                let payload = request.payload();
                self.payload.try_reserve_exact(payload.len()).map_err(|e| {
                    info!(
                        "Failed to reserve memory for program: {:?}",
                        Debug2Format(&e)
                    );
                    CoapError::internal_server_error()
                })?;
                self.payload.extend_from_slice(payload);

                if (block1_value & 0x8) == 0x8 {
                    Ok((block1, coap_numbers::code::CONTINUE))
                } else {
                    let image = core::mem::take(&mut self.payload);
                    SUIT_VERIFY_SIGNAL.signal(image.into_boxed_slice());
                    Ok((block1, coap_numbers::code::CHANGED))
                }
            }

            _ => Err(CoapError::method_not_allowed()),
        }
    }

    fn estimate_length(&mut self, _request: &Self::RequestData) -> usize {
        1
    }

    fn build_response<M: coap_message::MutableWritableMessage>(
        &mut self,
        response: &mut M,
        request: Self::RequestData,
    ) -> Result<(), Self::BuildResponseError<M>> {
        let (block1, code) = request;
        response.set_code(M::Code::new(code).map_err(CoapError::from_unionerror)?);

        if let Some(block1) = block1 {
            response
                .add_option_uint(
                    M::OptionNumber::new(coap_numbers::option::BLOCK1)
                        .map_err(CoapError::from_unionerror)?,
                    block1 as u32,
                )
                .map_err(CoapError::from_unionerror)?;
        }
        Ok(())
    }
}

pub async fn wait_for_update_request() -> Box<[u8]> {
    SUIT_VERIFY_SIGNAL.wait().await
}

#[ariel_os::task(autostart)]
async fn coap_task() {
    let control = VmControl::new();

    let handler = new_dispatcher()
        .at_with_attributes(&["vm-control"], &[], control)
        .with_wkc();

    info!("Starting CoAP handler");
    coap_run(handler).await;
}
