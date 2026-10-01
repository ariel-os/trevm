use core::net::SocketAddr;

use alloc::vec::Vec;
use ariel_os::time::Duration;
use coap_message::Code;
use coap_message::{MessageOption, MinimalWritableMessage, OptionNumber, ReadableMessage};
use coap_message_utils::OptionsExt;
use coap_request::Stack;
use coap_request_implementations::AsUriPath;

use ariel_os::reexports::embassy_time::with_timeout;

const BLOCK2_SZX: u32 = 6; // 2^(6 + 4) = 1024 bytes

#[derive(Debug)]
pub enum CoapFetchError {
    RequestFailed,
    TooLarge,
    Empty,
    Timeout,
    AllocationFailed { size: usize },
}

pub async fn get_blockwise(
    addr: SocketAddr,
    path: &str,
    max_size: usize,
    timeout: Duration,
) -> Result<Vec<u8>, CoapFetchError> {
    let client = ariel_os::coap::coap_client().await;

    let mut body = Vec::new();

    body.try_reserve_exact(max_size)
        .map_err(|_| CoapFetchError::AllocationFailed { size: max_size })?;
    let mut num = 0;

    loop {
        let more = with_timeout(
            timeout,
            client.to(addr).request(GetBlock2 {
                path,
                num,
                body: &mut body,
                max_size,
            }),
        )
        .await
        .map_err(|_| CoapFetchError::Timeout)?
        .map_err(|_| CoapFetchError::RequestFailed)??;

        if !more {
            break;
        }

        num += 1;
    }

    if body.is_empty() {
        return Err(CoapFetchError::Empty);
    }

    Ok(body)
}

fn block2_value(num: u32) -> u32 {
    // NUM | M=0 | SZX
    (num << 4) | BLOCK2_SZX
}

fn parse_block2(v: u32) -> Result<(u32, usize, bool), CoapFetchError> {
    let szx = v & 0x7;

    if szx > 6 {
        return Err(CoapFetchError::RequestFailed);
    }

    let num = v >> 4;
    let more = (v & 0x8) != 0;
    let size = 1usize << (4 + szx);

    Ok((num, size, more))
}

struct GetBlock2<'a> {
    path: &'a str,
    num: u32,
    body: &'a mut Vec<u8>,
    max_size: usize,
}

impl<S> coap_request::Request<S> for GetBlock2<'_>
where
    S: coap_request::Stack + ?Sized,
{
    type Carry = ();
    type Output = Result<bool, CoapFetchError>;

    async fn build_request(
        &mut self,
        req: &mut S::RequestMessage<'_>,
    ) -> Result<(), S::RequestUnionError> {
        let code =
            <S::RequestMessage<'_> as MinimalWritableMessage>::Code::new(coap_numbers::code::GET)
                .map_err(S::RequestMessage::convert_code_error)?;

        req.set_code(code);

        for part in self.path.as_uri_path() {
            req.add_option(
                <S::RequestMessage<'_> as MinimalWritableMessage>::OptionNumber::new(
                    coap_numbers::option::URI_PATH,
                )
                .map_err(S::RequestMessage::convert_option_number_error)?,
                part.as_bytes(),
            )
            .map_err(S::RequestMessage::convert_add_option_error)?;
        }

        req.add_option_uint(
            <S::RequestMessage<'_> as MinimalWritableMessage>::OptionNumber::new(
                coap_numbers::option::BLOCK2,
            )
            .map_err(S::RequestMessage::convert_option_number_error)?,
            block2_value(self.num),
        )
        .map_err(S::RequestMessage::convert_add_option_error)?;

        Ok(())
    }

    async fn process_response(&mut self, res: &S::ResponseMessage<'_>, _carry: ()) -> Self::Output {
        let code: u8 = res.code().into();

        if !matches!(
            coap_numbers::code::classify(code),
            coap_numbers::code::Range::Response(coap_numbers::code::Class::Success)
        ) {
            return Err(CoapFetchError::RequestFailed);
        }

        let mut block2 = None;

        res.options()
            .filter(|option| {
                if option.number() == coap_numbers::option::BLOCK2 {
                    block2 = option.value_uint();
                    false
                } else {
                    true
                }
            })
            .ignore_elective_others()
            .map_err(|_| CoapFetchError::RequestFailed)?;

        let payload = res.payload();

        let more = if let Some(block2) = block2 {
            let (num, size, more) = parse_block2(block2)?;

            if num != self.num || payload.len() > size {
                return Err(CoapFetchError::RequestFailed);
            }

            more
        } else {
            if self.num != 0 {
                return Err(CoapFetchError::RequestFailed);
            }

            false
        };

        let new_len = self
            .body
            .len()
            .checked_add(payload.len())
            .ok_or(CoapFetchError::TooLarge)?;

        if new_len > self.max_size {
            return Err(CoapFetchError::TooLarge);
        }

        self.body.extend_from_slice(payload);

        Ok(more)
    }
}
