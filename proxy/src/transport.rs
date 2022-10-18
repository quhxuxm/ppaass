use ppaass_protocol::PpaassProtocolAddress;

pub(crate) mod agent;
pub(crate) mod target;

pub(crate) enum TargetTcpTransportInputType {
    Connect { target_address: PpaassProtocolAddress },
    Relay { data: Vec<u8> },
}
pub(crate) struct TargetTcpTransportInput {
    input_type: TargetTcpTransportInputType,
}

impl TargetTcpTransportInput {
    pub(crate) fn new(input_type: TargetTcpTransportInputType) -> Self {
        Self { input_type }
    }
}
pub(crate) struct TargetTcpTransportOutput {}
