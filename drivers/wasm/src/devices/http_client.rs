use ::alloc::boxed::Box;
use alloc::{format, rc::Rc};
use core::cell::RefCell;
use file_system::{BaseOperations, CharacterDevice, Context, Error, MountOperations, Result, Size};
use shared::{HttpRequestParser, HttpResponseBuilder};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{Headers, Request, RequestInit};

enum State {
    Idle,
    RequestPending,
    RequestReady(web_sys::Response),
    BodyPending,
    BodyReady(js_sys::Uint8Array),
}

struct HttpClientContext {
    state: Rc<RefCell<State>>,
}

impl Clone for HttpClientContext {
    fn clone(&self) -> Self {
        HttpClientContext {
            state: Rc::new(RefCell::new(State::Idle)),
        }
    }
}

unsafe impl Send for HttpClientContext {}
unsafe impl Sync for HttpClientContext {}

fn build_request<'a>(scheme: &str, request: HttpRequestParser<'a>) -> Option<Request> {
    let options = RequestInit::new();
    let options_headers = Headers::new().ok()?;

    let mut host: Option<&str> = None;
    let mut path: Option<&str> = None;

    if let Some((m, p)) = request.get_request() {
        options.set_method(m);
        path = Some(p);
    }

    for item in request.get_headers() {
        let (name, value) = item;

        match name {
            HttpRequestParser::HOST_HEADER => {
                host = Some(value);
            }
            HttpRequestParser::CONNECTION_HEADER => {
                // Skip connection header
            }
            _ => {
                if let Err(e) = options_headers.append(name, value) {
                    log::warning!("Failed to append header {}: {:?}", name, e);
                }
            }
        }
    }

    if let Some(body) = request.get_body() {
        let concat_body = body.to_vec();
        options.set_body(&concat_body.into());
    }

    options.set_headers(&options_headers);

    let url = format!("{}://{}{}", scheme, host?, path.unwrap_or("/"));

    Request::new_with_str_and_init(&url, &options).ok()
}

fn build_headers_response(response: &web_sys::Response, buffer: &mut [u8]) -> Result<usize> {
    let mut builder = HttpResponseBuilder::from_buffer(buffer);

    // Status code
    let status = response.status();
    builder
        .add_status_code(status)
        .ok_or(Error::InternalError)?;

    // Headers
    for header in response.headers().entries() {
        let header = match header {
            Ok(h) => h,
            Err(_) => continue,
        };

        let header_array: js_sys::Array = header.dyn_into().map_err(|_| Error::InternalError)?;
        if header_array.length() != 2 {
            continue;
        }

        let name = header_array
            .get(0)
            .as_string()
            .ok_or(Error::InternalError)?;
        let value = header_array
            .get(1)
            .as_string()
            .ok_or(Error::InternalError)?;

        builder
            .add_header(&name, value.as_bytes())
            .ok_or(Error::InternalError)?;
    }

    // End of headers
    builder.add_line(b"").ok_or(Error::InternalError)?;

    Ok(builder.get_position())
}

pub struct HttpClientDevice;

unsafe impl Send for HttpClientDevice {}
unsafe impl Sync for HttpClientDevice {}

impl BaseOperations for HttpClientDevice {
    fn open(&self, context: &mut Context) -> Result<()> {
        context.set_private_data(Box::new(HttpClientContext {
            state: Rc::new(RefCell::new(State::Idle)),
        }));

        Ok(())
    }

    fn close(&self, context: &mut Context) -> Result<()> {
        context.take_private_data_of_type::<HttpClientContext>();

        Ok(())
    }

    fn read(&self, context: &mut Context, buffer: &mut [u8], _: Size) -> Result<usize> {
        let ctx = context
            .get_private_data_mutable_of_type::<HttpClientContext>()
            .ok_or(file_system::Error::InvalidParameter)?;

        let mut state = ctx.state.borrow_mut();

        match core::mem::replace(&mut *state, State::Idle) {
            State::RequestPending => {
                // Request is still in progress - restore state
                *state = State::RequestPending;
                Err(Error::RessourceBusy)
            }
            State::RequestReady(response) => {
                // Build headers response
                let headers_size = build_headers_response(&response, buffer)?;

                // Spawn body future
                let state_clone = ctx.state.clone();
                spawn_local(async move {
                    if let Ok(array_buffer_promise) = response.array_buffer() {
                        match JsFuture::from(array_buffer_promise).await {
                            Ok(body) => {
                                if let Ok(body) = body.dyn_into::<js_sys::ArrayBuffer>() {
                                    let array = js_sys::Uint8Array::new(&body);
                                    *state_clone.borrow_mut() = State::BodyReady(array);
                                } else {
                                    *state_clone.borrow_mut() = State::Idle;
                                }
                            }
                            Err(_) => {
                                *state_clone.borrow_mut() = State::Idle;
                            }
                        }
                    } else {
                        *state_clone.borrow_mut() = State::Idle;
                    }
                });

                *state = State::BodyPending;
                Ok(headers_size)
            }
            State::BodyPending => {
                // Body is still being fetched - restore state
                *state = State::BodyPending;
                log::information!("HttpClientDevice: body still pending");
                Err(Error::RessourceBusy)
            }
            State::BodyReady(body_array) => {
                let body_length = body_array.length() as usize;
                let read_length = core::cmp::min(body_length, buffer.len());

                body_array
                    .slice(0, read_length as u32)
                    .copy_to(&mut buffer[..read_length]);

                // State is already Idle from replace
                Ok(read_length)
            }
            State::Idle => {
                // No more data to read
                Ok(0)
            }
        }
    }

    fn write(&self, context: &mut Context, buffer: &[u8], _: Size) -> file_system::Result<usize> {
        let ctx = context
            .get_private_data_mutable_of_type::<HttpClientContext>()
            .ok_or(file_system::Error::InvalidParameter)?;

        let request_parser = HttpRequestParser::from_buffer(buffer);
        let request =
            build_request("https", request_parser).ok_or(file_system::Error::InvalidParameter)?;

        // Spawn the request
        let state_clone = ctx.state.clone();
        spawn_local(async move {
            let window = web_sys::window().unwrap();
            match JsFuture::from(window.fetch_with_request(&request)).await {
                Ok(response) => {
                    if let Ok(response) = response.dyn_into::<web_sys::Response>() {
                        *state_clone.borrow_mut() = State::RequestReady(response);
                    }
                }
                Err(_) => {
                    *state_clone.borrow_mut() = State::Idle;
                }
            }
        });

        *ctx.state.borrow_mut() = State::RequestPending;

        Ok(buffer.len())
    }

    fn clone_context(&self, context: &Context) -> file_system::Result<Context> {
        let context = context
            .get_private_data_of_type::<HttpClientContext>()
            .ok_or(file_system::Error::InvalidParameter)?;

        Ok(Context::new(Some(context.clone())))
    }
}

impl MountOperations for HttpClientDevice {}

impl CharacterDevice for HttpClientDevice {}
