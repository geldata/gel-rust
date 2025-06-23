use std::sync::Arc;

use bytes::{Bytes, BytesMut};
use tokio::time::Instant;

use gel_errors::fields::QueryText;
use gel_protocol::client_message::{ClientMessage, Parse};
use gel_protocol::client_message::Execute1;
use gel_protocol::common::CompilationOptions;
use gel_protocol::common::{Capabilities, Cardinality, InputLanguage, IoFormat};
use gel_protocol::descriptors::Typedesc;
use gel_protocol::encoding::Annotations;
use gel_protocol::features::ProtocolVersion;
use gel_protocol::model::Uuid;
use gel_protocol::query_arg::{Encoder, QueryArgs};
use gel_protocol::server_message::{CommandDataDescription1, Data, ServerMessage};
use gel_protocol::QueryResult;

use crate::errors::NoResultExpected;
use crate::errors::{ClientConnectionEosError, ProtocolEncodingError};
use crate::errors::{ClientInconsistentError, ProtocolOutOfOrderError};
use crate::errors::{Error, ErrorKind};
use crate::raw::connection::Mode;
use crate::raw::{Connection, PoolConnection, QueryCapabilities};
use crate::raw::{Description, Response, ResponseStream, State};

pub(crate) struct Guard;

impl Connection {
    pub(crate) fn begin_request(&mut self) -> Result<Guard, Error> {
        match self.mode {
            Mode::Normal { .. } => {
                self.mode = Mode::Dirty;
                Ok(Guard)
            }
            Mode::Dirty => Err(ClientInconsistentError::build()),
            // TODO(tailhook) technically we could just wait ping here
            Mode::AwaitingPing => Err(ClientInconsistentError::with_message("interrupted ping")),
        }
    }
    pub(crate) fn end_request(&mut self, _guard: Guard) {
        self.mode = Mode::Normal {
            idle_since: Instant::now(),
        };
    }
    pub(crate) async fn expect_ready(&mut self, guard: Guard) -> Result<(), Error> {
        loop {
            let msg = self.message().await?;

            // TODO(tailhook) should we react on messages somehow?
            //                At least parse LogMessage's?

            if let ServerMessage::ReadyForCommand(ready) = msg {
                self.transaction_state = ready.transaction_state;
                self.end_request(guard);
                return Ok(());
            }
        }
    }

    pub(crate) async fn expect_ready_or_eos(&mut self, guard: Guard) -> Result<(), Error> {
        match self.expect_ready(guard).await {
            Ok(()) => Ok(()),
            Err(e) if e.is::<ClientConnectionEosError>() => {
                assert!(!self.is_consistent());
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
    pub async fn parse(
        &mut self,
        flags: &CompilationOptions,
        query: &str,
        state: &dyn State,
        annotations: &Arc<Annotations>,
    ) -> Result<CommandDataDescription1, Error> {
        self._parse1(flags, query, state, annotations)
            .await
            .map_err(|e| e.set::<QueryText>(query))
    }
    async fn _parse1(
        &mut self,
        flags: &CompilationOptions,
        query: &str,
        state: &dyn State,
        annotations: &Arc<Annotations>,
    ) -> Result<CommandDataDescription1, Error> {
        let guard = self.begin_request()?;
        self.send_messages(&[
            ClientMessage::Parse(Parse::new(
                flags,
                query,
                state.encode(&self.state_desc)?,
                self.proto.is_3().then(|| annotations.clone()),
            )),
            ClientMessage::Sync,
        ])
        .await?;

        loop {
            let msg = self.message().await?;
            match msg {
                ServerMessage::StateDataDescription(d) => {
                    self.state_desc = d.typedesc;
                }
                ServerMessage::CommandDataDescription1(data_desc) => {
                    self.expect_ready(guard).await?;
                    return Ok(data_desc);
                }
                ServerMessage::ErrorResponse(err) => {
                    self.expect_ready_or_eos(guard)
                        .await
                        .map_err(|e| log::warn!("Error waiting for Ready after error: {e:#}"))
                        .ok();
                    return Err(err.into());
                }
                _ => {
                    return Err(ProtocolOutOfOrderError::with_message(format!(
                        "Unsolicited message {msg:?}"
                    )));
                }
            }
        }
    }

    async fn _execute(
        &mut self,
        opts: &CompilationOptions,
        query: &str,
        state: &dyn State,
        annotations: &Arc<Annotations>,
        desc: &CommandDataDescription1,
        arguments: &Bytes,
    ) -> Result<Response<Vec<Data>>, Error> {
        self._execute1(opts, query, state, annotations, desc, arguments)
            .await
            .map_err(|e| e.set::<QueryText>(query))
    }

    async fn _execute1(
        &mut self,
        opts: &CompilationOptions,
        query: &str,
        state: &dyn State,
        annotations: &Arc<Annotations>,
        desc: &CommandDataDescription1,
        arguments: &Bytes,
    ) -> Result<Response<Vec<Data>>, Error> {
        let guard = self.begin_request()?;
        self.send_messages(&[
            ClientMessage::Execute1(Execute1 {
                annotations: self.proto.is_3().then(|| annotations.clone()),
                allowed_capabilities: opts.allow_capabilities,
                compilation_flags: opts.flags(),
                implicit_limit: opts.implicit_limit,
                input_language: opts.input_language,
                output_format: opts.io_format,
                expected_cardinality: opts.expected_cardinality,
                command_text: query.into(),
                state: state.encode(&self.state_desc)?,
                input_typedesc_id: desc.input.id,
                output_typedesc_id: desc.output.id,
                arguments: arguments.clone(),
            }),
            ClientMessage::Sync,
        ])
        .await?;

        let mut data = Vec::new();
        let mut description = None;
        let mut warnings: Vec<gel_protocol::annotations::Warning> = Vec::new();
        loop {
            let msg = self.message().await?;
            match msg {
                ServerMessage::StateDataDescription(d) => {
                    self.state_desc = d.typedesc;
                }
                ServerMessage::CommandDataDescription1(desc) => {
                    warnings.extend(gel_protocol::annotations::decode_warnings(
                        &desc.annotations,
                    )?);
                    description = Some(desc);
                }
                ServerMessage::Data(datum) => {
                    data.push(datum);
                }
                ServerMessage::CommandComplete1(complete) => {
                    self.expect_ready(guard).await?;
                    return Ok(Response {
                        new_state: complete.state,
                        warnings,
                        ..Response::new(complete.status, data)
                    });
                }
                ServerMessage::ErrorResponse(err) => {
                    self.expect_ready_or_eos(guard)
                        .await
                        .map_err(|e| log::warn!("Error waiting for Ready after error: {e:#}"))
                        .ok();
                    let mut err: Error = err.into();
                    if let Some(desc) = description {
                        err = err.set::<Description>(desc);
                    }
                    return Err(err);
                }
                _ => {
                    return Err(ProtocolOutOfOrderError::with_message(format!(
                        "Unsolicited message {msg:?}"
                    )));
                }
            }
        }
    }

    pub async fn execute_stream<R, A>(
        &mut self,
        opts: &CompilationOptions,
        query: &str,
        state: &dyn State,
        annotations: &Arc<Annotations>,
        desc: &CommandDataDescription1,
        arguments: &A,
    ) -> Result<ResponseStream<R>, Error>
    where
        A: QueryArgs,
        R: QueryResult,
        R::State: Unpin,
    {
        let inp_desc = desc.input().map_err(ProtocolEncodingError::with_source)?;

        let mut arg_buf = BytesMut::with_capacity(8);
        arguments.encode(&mut Encoder::new(
            &inp_desc.as_query_arg_context(),
            &mut arg_buf,
        ))?;

        let guard = self.begin_request()?;
        self.send_messages(&[
            ClientMessage::Execute1(Execute1 {
                annotations: self.proto.is_3().then(|| annotations.clone()),
                allowed_capabilities: opts.allow_capabilities,
                compilation_flags: opts.flags(),
                implicit_limit: opts.implicit_limit,
                input_language: opts.input_language,
                output_format: opts.io_format,
                expected_cardinality: opts.expected_cardinality,
                command_text: query.into(),
                state: state.encode(&self.state_desc)?,
                input_typedesc_id: desc.input.id,
                output_typedesc_id: desc.output.id,
                arguments: arg_buf.freeze(),
            }),
            ClientMessage::Sync,
        ])
        .await?;

        let out_desc = desc.output().map_err(ProtocolEncodingError::with_source)?;
        ResponseStream::new(self, &out_desc, guard).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn try_execute_stream<R, A>(
        &mut self,
        opts: &CompilationOptions,
        query: &str,
        state: &dyn State,
        annotations: &Arc<Annotations>,
        input: &Typedesc,
        output: &Typedesc,
        arguments: &A,
    ) -> Result<ResponseStream<R>, Error>
    where
        A: QueryArgs,
        R: QueryResult,
        R::State: Unpin,
    {
        let mut arg_buf = BytesMut::with_capacity(8);
        arguments.encode(&mut Encoder::new(
            &input.as_query_arg_context(),
            &mut arg_buf,
        ))?;

        let guard = self.begin_request()?;
        self.send_messages(&[
            ClientMessage::Execute1(Execute1 {
                annotations: self.proto.is_3().then(|| annotations.clone()),
                allowed_capabilities: opts.allow_capabilities,
                compilation_flags: opts.flags(),
                implicit_limit: opts.implicit_limit,
                input_language: opts.input_language,
                output_format: opts.io_format,
                expected_cardinality: opts.expected_cardinality,
                command_text: query.into(),
                state: state.encode(&self.state_desc)?,
                input_typedesc_id: *input.id(),
                output_typedesc_id: *input.id(),
                arguments: arg_buf.freeze(),
            }),
            ClientMessage::Sync,
        ])
        .await?;

        ResponseStream::new(self, output, guard).await
    }
    pub async fn statement(
        &mut self,
        flags: &CompilationOptions,
        query: &str,
        state: &dyn State,
        annotations: &Arc<Annotations>,
    ) -> Result<(), Error> {
        self._statement1(flags, query, state, annotations).await
    }

    async fn _statement1(
        &mut self,
        opts: &CompilationOptions,
        query: &str,
        state: &dyn State,
        annotations: &Arc<Annotations>,
    ) -> Result<(), Error> {
        let guard = self.begin_request()?;
        self.send_messages(&[
            ClientMessage::Execute1(Execute1 {
                annotations: self.proto.is_3().then(|| annotations.clone()),
                allowed_capabilities: opts.allow_capabilities,
                compilation_flags: opts.flags(),
                implicit_limit: opts.implicit_limit,
                input_language: opts.input_language,
                output_format: opts.io_format,
                expected_cardinality: opts.expected_cardinality,
                command_text: query.into(),
                state: state.encode(&self.state_desc)?,
                input_typedesc_id: Uuid::from_u128(0),
                output_typedesc_id: Uuid::from_u128(0),
                arguments: Bytes::new(),
            }),
            ClientMessage::Sync,
        ])
        .await?;

        loop {
            let msg = self.message().await?;
            match msg {
                ServerMessage::StateDataDescription(d) => {
                    self.state_desc = d.typedesc;
                }
                ServerMessage::Data(_) => {}
                ServerMessage::CommandComplete1(..) => {
                    self.expect_ready(guard).await?;
                    return Ok(());
                }
                ServerMessage::ErrorResponse(err) => {
                    self.expect_ready_or_eos(guard)
                        .await
                        .map_err(|e| log::warn!("Error waiting for Ready after error: {e:#}"))
                        .ok();
                    return Err(err.into());
                }
                _ => {
                    return Err(ProtocolOutOfOrderError::with_message(format!(
                        "Unsolicited message {msg:?}"
                    )));
                }
            }
        }
    }
    
    #[allow(clippy::too_many_arguments)]
    pub async fn query<R, A>(
        &mut self,
        query: &str,
        arguments: &A,
        state: &dyn State,
        annotations: &Arc<Annotations>,
        allow_capabilities: Capabilities,
        io_format: IoFormat,
        cardinality: Cardinality,
    ) -> Result<Response<Vec<R>>, Error>
    where
        A: QueryArgs,
        R: QueryResult,
    {
        let mut caps = QueryCapabilities::Unparsed;
        let result = async {
            let flags = CompilationOptions {
                implicit_limit: None,
                implicit_typenames: false,
                implicit_typeids: false,
                explicit_objectids: true,
                allow_capabilities,
                io_format,
                input_language: InputLanguage::EdgeQL,
                expected_cardinality: cardinality,
            };
            let desc = self.parse(&flags, query, state, annotations).await?;
            caps = QueryCapabilities::Parsed(desc.capabilities);
            let inp_desc = desc.input().map_err(ProtocolEncodingError::with_source)?;

            let mut arg_buf = BytesMut::with_capacity(8);
            if let Err(e) = arguments.encode(&mut Encoder::new(
                &inp_desc.as_query_arg_context(),
                &mut arg_buf,
            )) {
                return Err(e.set::<Description>(desc));
            }

            let response = self
                ._execute(&flags, query, state, annotations, &desc, &arg_buf.freeze())
                .await?;
            response.log_warnings();

            let out_desc = desc.output().map_err(ProtocolEncodingError::with_source)?;
            match out_desc.root_pos() {
                Some(root_pos) => {
                    let ctx = out_desc.as_queryable_context();
                    let mut state = R::prepare(&ctx, root_pos)?;
                    response.map(|data| {
                        data.into_iter()
                            .flat_map(|chunk| chunk.data)
                            .map(|chunk| R::decode(&mut state, &chunk))
                            .collect::<Result<Vec<_>, _>>()
                    })
                }
                None => Err(NoResultExpected::build()),
            }
        }
        .await;
        result.map_err(|e| e.set::<QueryCapabilities>(caps))
    }

    pub async fn execute<A>(
        &mut self,
        query: &str,
        arguments: &A,
        state: &dyn State,
        annotations: &Arc<Annotations>,
        allow_capabilities: Capabilities,
    ) -> Result<Response<()>, Error>
    where
        A: QueryArgs,
    {
        let mut caps = QueryCapabilities::Unparsed;
        let result: Result<_, Error> = async {
            let flags = CompilationOptions {
                implicit_limit: None,
                implicit_typenames: false,
                implicit_typeids: false,
                explicit_objectids: true,
                allow_capabilities,
                input_language: InputLanguage::EdgeQL,
                io_format: IoFormat::Binary,
                expected_cardinality: Cardinality::Many,
            };
            let desc = self.parse(&flags, query, state, annotations).await?;
            caps = QueryCapabilities::Parsed(desc.capabilities);
            let inp_desc = desc.input().map_err(ProtocolEncodingError::with_source)?;

            let mut arg_buf = BytesMut::with_capacity(8);
            if let Err(e) = arguments.encode(&mut Encoder::new(
                &inp_desc.as_query_arg_context(),
                &mut arg_buf,
            )) {
                return Err(e.set::<Description>(desc));
            }

            let response = self
                ._execute(&flags, query, state, annotations, &desc, &arg_buf.freeze())
                .await?;
            response.log_warnings();
            response.map(|_| Ok::<_, Error>(()))
        }
        .await;
        result.map_err(|e| e.set::<QueryCapabilities>(caps))
    }
}

impl PoolConnection {
    pub async fn parse(
        &mut self,
        flags: &CompilationOptions,
        query: &str,
        state: &dyn State,
        annotations: &Arc<Annotations>,
    ) -> Result<CommandDataDescription1, Error> {
        self.inner().parse(flags, query, state, annotations).await
    }
    pub async fn execute(
        &mut self,
        opts: &CompilationOptions,
        query: &str,
        state: &dyn State,
        annotations: &Arc<Annotations>,
        desc: &CommandDataDescription1,
        arguments: &Bytes,
    ) -> Result<Vec<Data>, Error> {
        self.inner()
            ._execute(opts, query, state, annotations, desc, arguments)
            .await
            .map(|r| r.data)
    }
    pub async fn statement(
        &mut self,
        query: &str,
        state: &dyn State,
        annotations: &Arc<Annotations>,
    ) -> Result<(), Error> {
        let flags = CompilationOptions {
            implicit_limit: None,
            implicit_typenames: false,
            implicit_typeids: false,
            explicit_objectids: false,
            allow_capabilities: Capabilities::ALL,
            input_language: InputLanguage::EdgeQL,
            io_format: IoFormat::Binary,
            expected_cardinality: Cardinality::Many, // no result is unsupported
        };
        self.inner().statement(&flags, query, state, annotations).await
    }
    pub fn proto(&self) -> &ProtocolVersion {
        &self
            .inner
            .as_ref()
            .expect("connection is not dropped")
            .proto
    }
    pub fn inner(&mut self) -> &mut Connection {
        self.inner.as_mut().expect("connection is not dropped")
    }
}
