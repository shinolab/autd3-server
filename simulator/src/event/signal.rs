use autd3_driver::{firmware::cpu::TxMessage, geometry::Geometry};

pub enum Signal {
    ConfigGeometry(Geometry),
    UpdateGeometry(Geometry),
    Send(Vec<TxMessage>),
    Close,
}

impl std::fmt::Debug for Signal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Signal::ConfigGeometry(_) => write!(f, "ConfigGeometry"),
            Signal::UpdateGeometry(_) => write!(f, "UpdateGeometry"),
            Signal::Send(tx) => write!(f, "Send({:?})", tx),
            Signal::Close => write!(f, "Close"),
        }
    }
}
