use m3::vec::Vec;
use smoltcp::Result;

//ToDo: Using u8 as a placeholder for socket handle for now, replace later
pub type Messages = Vec<(u8, Vec<u8>)>;

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum InterfaceCall{
    InitPoll,
    InitIngress,
    // ToDo: This represents the received data, the receive result and the tx token but probably there is a nicer way to do this.
    ProcessIngress(Option<(Vec<u8>, Result<()>, Option<()>)>),
    LoopIngress(Result<()>),
    InitEgress,
    PollLoopCondition,
    // Todo: To avoid the hassle with dyn Device::Token we use a simple ok for
    //  now to represent the token
    InnerDispatchLocal(Option<()>),
    // takes the DeviceResult
    MatchSocketDispatchAfter(Result<()>),
    // takes the emit result of the current socket
    HandleResult(Result<()>),
    // iterates to nex socket & packet
    UpdateEgressState,
    AnswerToSocket(Vec<(u8, Vec<u8>)>),
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum InterfaceState{
    Egress,
    Ingress
}

