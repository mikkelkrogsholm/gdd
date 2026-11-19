//! Serve HTTP/1 on loopback and finish admitted requests before returning.

use std::{
    io::{self, Write},
    net::Shutdown,
    rc::Rc,
};

use ntex::{
    ServiceFactory, SharedCfg,
    http::{
        HttpService, HttpServiceConfig, Response, ResponseError,
        error::DecodeError,
        h1::{Control, ProtocolError, control::Reason},
    },
};
use tokio::{net::TcpListener, task::JoinSet};

use crate::api::{self, App};

// The previous native transport allowed 100 header fields and a 408 KiB head.
const HEADER_FIELDS: usize = 100;
const MAX_HEAD_BYTES: usize = 408 * 1024;

pub async fn serve(
    listener: TcpListener,
    app: App,
    shutdown: impl Future<Output = ()>,
) -> io::Result<()> {
    let app = Rc::new(app);
    let config = SharedCfg::new("local-sessions")
        .add(
            HttpServiceConfig::new()
                .set_max_headers(HEADER_FIELDS)
                .set_max_buf_size(MAX_HEAD_BYTES),
        )
        .build();
    let admission = Rc::clone(&app);
    let service = HttpService::h1(move |request| {
        let app = Rc::clone(&app);
        async move { app.handle(request).await }
    })
    .control(move |message: Control<_, _>| {
        let result = match message {
            // The parser counts distinct names. Each repeated field counts towards admission.
            Control::Request(req)
                if req.get_ref().headers().iter().nth(HEADER_FIELDS).is_some() =>
            {
                req.fail_with(
                    Response::RequestHeaderFieldsTooLarge()
                        .force_close()
                        .finish(),
                )
            }
            Control::Expect(expect) => match admission.validate_head(expect.get_ref()) {
                Ok(_) => expect.ack(),
                Err(error) => expect.fail_with(error.error_response()),
            },
            Control::Disconnect(Reason::ProtocolError(error))
                if matches!(
                    error.err(),
                    ProtocolError::Decode(DecodeError::Header | DecodeError::InvalidInput(_))
                ) =>
            {
                error.fail_with(api::invalid_framing())
            }
            event => event.ack(),
        };
        std::future::ready(Ok::<_, io::Error>(result))
    })
    .pipeline(config.clone())
    .await
    .map_err(|()| io::Error::other("Cannot initialize HTTP service"))?;

    println!("Local Sessions: http://{}", listener.local_addr()?);
    io::stdout().flush()?;
    let mut connections = JoinSet::new();
    tokio::pin!(shutdown);
    let result = loop {
        tokio::select! {
            _ = &mut shutdown => break Ok(()),
            Some(_) = connections.join_next() => (),
            incoming = listener.accept() => {
                let (stream, _) = match incoming {
                    Ok(pair) => pair,
                    Err(error) => break Err(error),
                };
                let service = service.clone();
                let config = config.clone();
                connections.spawn_local(async move {
                    let socket = stream.into_std()?;
                    let io = ntex::rt::from_tcp_stream(socket.try_clone()?, config)?;
                    let _ = service.call(io).await;
                    // Finish the response side before closing a socket that may still
                    // have unread input, for example after rejecting an oversized body.
                    socket.shutdown(Shutdown::Write)
                });
            }
        }
    };
    // Closing the listener rejects new connections. The HTTP driver closes idle
    // connections and finishes the current request on each active connection.
    drop(listener);
    service.shutdown().await;
    while connections.join_next().await.is_some() {}
    result
}
