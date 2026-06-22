// SPDX-FileCopyrightText: 2025 Sven Shi
// SPDX-License-Identifier: GPL-3.0-or-later

//! TCP nameserver client.

use async_trait::async_trait;

use super::super::endpoint::NameserverConfig;
use super::{NameserverClient, effective_deadline};
use crate::infra::error::Result;
use crate::infra::network::deadline::{DeadlineOutcome, QueryDeadline};
use crate::infra::network::dial::SocketOptions;
use crate::infra::network::proxy::connect_tcp as proxy_connect_tcp;
use crate::infra::network::transport::tcp_transport::{TcpTransportReader, TcpTransportWriter};
use crate::proto::Message;

#[derive(Debug)]
pub(super) struct TcpNameserverClient {
    config: NameserverConfig,
}

impl TcpNameserverClient {
    pub(super) fn new(config: NameserverConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl NameserverClient for TcpNameserverClient {
    async fn query(&self, request: Message, deadline: QueryDeadline) -> Result<Message> {
        query_tcp_config(
            &self.config,
            request,
            effective_deadline(deadline, self.config.timeout),
        )
        .await
    }

    fn label(&self) -> &str {
        self.config.label.as_str()
    }
}

pub(super) async fn query_tcp_config(
    config: &NameserverConfig,
    request: Message,
    deadline: QueryDeadline,
) -> Result<Message> {
    let stream = match deadline
        .run(proxy_connect_tcp(
            config.target(),
            SocketOptions::default(),
            config.socks5.clone(),
        ))
        .await
    {
        DeadlineOutcome::Completed(result) => result?,
        DeadlineOutcome::Expired => return Err(deadline.timeout_error()),
    };
    let (reader, writer) = stream.into_split();
    query_framed_tcp(
        TcpTransportReader::new(reader),
        TcpTransportWriter::new(writer),
        request,
        deadline,
    )
    .await
}

pub(super) async fn query_framed_tcp<R, W>(
    mut reader: TcpTransportReader<R>,
    mut writer: TcpTransportWriter<W>,
    request: Message,
    deadline: QueryDeadline,
) -> Result<Message>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let query_id = request.id();
    match deadline.run(writer.write_message(&request)).await {
        DeadlineOutcome::Completed(result) => result?,
        DeadlineOutcome::Expired => return Err(deadline.timeout_error()),
    }
    let mut response = match deadline.run(reader.read_message()).await {
        DeadlineOutcome::Completed(result) => result?,
        DeadlineOutcome::Expired => return Err(deadline.timeout_error()),
    };
    response.set_id(query_id);
    Ok(response)
}
