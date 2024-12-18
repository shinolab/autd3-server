use std::sync::Arc;

use autd3_protobuf::{
    simulator_server, CloseRequest, CloseResponse, FromMessage, Geometry, GeometryResponse,
    ReadRequest, RxMessage, SendResponse, TxRawData,
};
use parking_lot::RwLock;
use tonic::{Request, Response, Status};
use winit::event_loop::EventLoopProxy;

use crate::event::{Signal, UserEvent};

pub struct SimulatorServer {
    pub rx_buf: Arc<RwLock<Vec<autd3_driver::firmware::cpu::RxMessage>>>,
    pub proxy: EventLoopProxy<UserEvent>,
}

#[tonic::async_trait]
impl simulator_server::Simulator for SimulatorServer {
    async fn config_geomety(
        &self,
        req: Request<Geometry>,
    ) -> Result<Response<GeometryResponse>, Status> {
        let geometry = autd3_driver::geometry::Geometry::from_msg(&req.into_inner())?;
        if self
            .proxy
            .send_event(UserEvent::Server(Signal::ConfigGeometry(geometry)))
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
        let geometry = autd3_driver::geometry::Geometry::from_msg(&req.into_inner())?;
        if self
            .proxy
            .send_event(UserEvent::Server(Signal::UpdateGeometry(geometry)))
            .is_err()
        {
            return Err(Status::unavailable("Simulator is closed"));
        }
        Ok(Response::new(GeometryResponse {}))
    }

    async fn send_data(&self, req: Request<TxRawData>) -> Result<Response<SendResponse>, Status> {
        let tx = Vec::<autd3_driver::firmware::cpu::TxMessage>::from_msg(&req.into_inner())?;
        if self
            .proxy
            .send_event(UserEvent::Server(Signal::Send(tx)))
            .is_err()
        {
            return Err(Status::unavailable("Simulator is closed"));
        }
        Ok(Response::new(SendResponse { success: true }))
    }

    async fn read_data(&self, _: Request<ReadRequest>) -> Result<Response<RxMessage>, Status> {
        let rx = self.rx_buf.read();
        Ok(Response::new(RxMessage {
            data: rx.iter().flat_map(|c| [c.data(), c.ack()]).collect(),
        }))
    }

    async fn close(&self, _: Request<CloseRequest>) -> Result<Response<CloseResponse>, Status> {
        if self
            .proxy
            .send_event(UserEvent::Server(Signal::Close))
            .is_err()
        {
            return Err(Status::unavailable("Simulator is closed"));
        }
        Ok(Response::new(CloseResponse { success: true }))
    }
}
