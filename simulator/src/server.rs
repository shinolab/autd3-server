use std::{
    error::Error,
    net::ToSocketAddrs,
    sync::{Arc, RwLock},
};

use autd3_driver::firmware::cpu::TxDatagram;
use crossbeam_channel::{bounded, Receiver, Sender, TryRecvError};
use futures_util::future::FutureExt;
use tokio::{runtime::Builder, sync::oneshot};
use tonic::{transport::Server, Request, Response, Status};

use autd3_protobuf::*;

use crate::{
    context::Context, renderer::Renderer, state::State, update_flag::UpdateFlag, SimulatorError,
};

enum Signal {
    ConfigGeometry(Geometry),
    UpdateGeometry(Geometry),
    Send(TxRawData),
    Close,
}

struct SimulatorServer {
    rx_buf: Arc<RwLock<Vec<autd3_driver::firmware::cpu::RxMessage>>>,
    sender: Sender<Signal>,
}

#[tonic::async_trait]
impl simulator_server::Simulator for SimulatorServer {
    async fn config_geomety(
        &self,
        req: Request<Geometry>,
    ) -> Result<Response<GeometryResponse>, Status> {
        if self
            .sender
            .send(Signal::ConfigGeometry(req.into_inner()))
            .is_err()
        {
            return Err(Status::unavailable("Simulator is closed"));
        }
        Ok(Response::new(GeometryResponse {}))
    }

    async fn update_geomety(
        &self,
        req: Request<Geometry>,
    ) -> Result<Response<GeometryResponse>, Status> {
        if self
            .sender
            .send(Signal::UpdateGeometry(req.into_inner()))
            .is_err()
        {
            return Err(Status::unavailable("Simulator is closed"));
        }
        Ok(Response::new(GeometryResponse {}))
    }

    async fn send_data(&self, req: Request<TxRawData>) -> Result<Response<SendResponse>, Status> {
        if self.sender.send(Signal::Send(req.into_inner())).is_err() {
            return Err(Status::unavailable("Simulator is closed"));
        }
        Ok(Response::new(SendResponse { success: true }))
    }

    async fn read_data(&self, _: Request<ReadRequest>) -> Result<Response<RxMessage>, Status> {
        let rx = self.rx_buf.read().unwrap();
        Ok(Response::new(RxMessage {
            data: rx.iter().flat_map(|c| [c.data(), c.ack()]).collect(),
        }))
    }

    async fn close(&self, _: Request<CloseRequest>) -> Result<Response<CloseResponse>, Status> {
        if self.sender.send(Signal::Close).is_err() {
            return Err(Status::unavailable("Simulator is closed"));
        }
        Ok(Response::new(CloseResponse { success: true }))
    }
}

#[allow(clippy::type_complexity)]
pub struct ServerWrapper {
    server_th: std::thread::JoinHandle<Result<(), tonic::transport::Error>>,
    rx_buf: Arc<RwLock<Vec<autd3_driver::firmware::cpu::RxMessage>>>,
    receiver: Receiver<Signal>,
    shutdown: tokio::sync::oneshot::Sender<()>,
    lightweight_server: Option<(
        std::thread::JoinHandle<Result<(), tonic::transport::Error>>,
        tokio::sync::oneshot::Sender<()>,
    )>,
}

impl ServerWrapper {
    pub fn new(port: u16, lightweight: bool, lightweight_port: u16) -> Self {
        let (tx, rx) = bounded(32);

        let (tx_shutdown, rx_shutdown) = oneshot::channel::<()>();

        let rx_buf = Arc::new(RwLock::new(vec![]));
        let server_th = std::thread::spawn({
            let rx_buf = rx_buf.clone();
            move || {
                tracing::info!("Waiting for client connection on http://0.0.0.0:{}", port);
                Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(async {
                        Server::builder()
                            .add_service(simulator_server::SimulatorServer::new(SimulatorServer {
                                rx_buf,
                                sender: tx,
                            }))
                            .serve_with_shutdown(
                                format!("0.0.0.0:{port}")
                                    .to_socket_addrs()
                                    .unwrap()
                                    .next()
                                    .unwrap(),
                                rx_shutdown.map(drop),
                            )
                            .await
                    })
            }
        });

        let lightweight_server = if lightweight {
            let (tx_shutdown_lightweigh, rx_shutdown_lightweight) = oneshot::channel::<()>();
            Some((
                std::thread::spawn({
                    move || {
                        use autd3_protobuf::{lightweight::LightweightServer, *};
                        Builder::new_multi_thread()
                            .enable_all()
                            .build()
                            .unwrap()
                            .block_on(async {
                                let server = LightweightServer::new(move || {
                                    autd3_link_simulator::Simulator::builder(
                                        format!("127.0.0.1:{}", port).parse().unwrap(),
                                    )
                                });
                                Server::builder()
                                    .add_service(ecat_light_server::EcatLightServer::new(server))
                                    .serve_with_shutdown(
                                        format!("0.0.0.0:{}", lightweight_port)
                                            .to_socket_addrs()
                                            .unwrap()
                                            .next()
                                            .unwrap(),
                                        rx_shutdown_lightweight.map(drop),
                                    )
                                    .await
                            })
                    }
                }),
                tx_shutdown_lightweigh,
            ))
        } else {
            None
        };

        Self {
            server_th,
            rx_buf,
            receiver: rx,
            shutdown: tx_shutdown,
            lightweight_server,
        }
    }

    pub fn update(
        &mut self,
        update_flag: &mut UpdateFlag,
        renderer: &mut Renderer,
        context: &Context,
        state: &mut State,
    ) -> Result<(), SimulatorError> {
        if state
            .cpus
            .iter()
            .any(autd3_firmware_emulator::CPUEmulator::should_update)
        {
            self.rx_buf
                .write()
                .unwrap()
                .iter_mut()
                .zip(state.cpus.iter())
                .for_each(|(d, s)| {
                    *d = s.rx();
                });
        }

        match self.receiver.try_recv() {
            Ok(Signal::ConfigGeometry(geometry)) => {
                state.clear();

                let geometry = autd3_driver::geometry::Geometry::from_msg(&geometry)?;
                *self.rx_buf.write().unwrap() =
                    vec![autd3_driver::firmware::cpu::RxMessage::new(0, 0); geometry.num_devices()];

                state.init(geometry);
                renderer.init(context, state)?;

                update_flag.set(UpdateFlag::UPDATE_CAMERA, true);
                update_flag.set(UpdateFlag::UPDATE_TRANS_POS, true);
                update_flag.set(UpdateFlag::UPDATE_TRANS_ALPHA, true);
                update_flag.set(UpdateFlag::UPDATE_TRANS_STATE, true);
                update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                update_flag.set(UpdateFlag::UPDATE_SLICE_SIZE, true);
                update_flag.set(UpdateFlag::UPDATE_SLICE_COLOR_MAP, true);
                update_flag.set(UpdateFlag::UPDATE_CONFIG, true);
            }
            Ok(Signal::UpdateGeometry(geometry)) => {
                let geometry = autd3_driver::geometry::Geometry::from_msg(&geometry)?;
                state.update_geometry(geometry);

                update_flag.set(UpdateFlag::UPDATE_TRANS_POS, true);
            }
            Ok(Signal::Send(raw)) => {
                let tx = TxDatagram::from_msg(&raw)?;
                state.cpus.iter_mut().for_each(|cpu| {
                    cpu.send(&tx);
                });
                self.rx_buf
                    .write()
                    .unwrap()
                    .iter_mut()
                    .zip(state.cpus.iter())
                    .for_each(|(d, s)| {
                        *d = s.rx();
                    });

                update_flag.set(UpdateFlag::UPDATE_TRANS_STATE, true);
            }
            Ok(Signal::Close) => {
                state.clear();
            }
            Err(TryRecvError::Empty) => {}
            _ => {}
        }

        Ok(())
    }

    pub fn shutdown(self) {
        let Self {
            server_th,
            shutdown,
            lightweight_server,
            ..
        } = self;

        if let Some((server_th, tx_shutdown_lightweigh)) = lightweight_server {
            let _ = tx_shutdown_lightweigh.send(());
            if let Err(e) = server_th.join().unwrap() {
                match e.source() {
                    Some(e) => tracing::error!("Server error: {}", e),
                    None => tracing::error!("Server error: {}", e),
                }
            }
        }

        let _ = shutdown.send(());
        if let Err(e) = server_th.join().unwrap() {
            match e.source() {
                Some(e) => tracing::error!("Server error: {}", e),
                None => tracing::error!("Server error: {}", e),
            }
        }
    }
}
