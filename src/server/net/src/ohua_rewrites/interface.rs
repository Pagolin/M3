use m3::vec::Vec;

use super::tcp_socket::{ReprOwned as TcpReprP};
use smoltcp::Result;
use smoltcp::wire::*;
// use smoltcp::socket::*;
use smoltcp::iface::{SocketSet, SocketHandle, SocketStorage};
// Things that used to be private to the original interface but we make it public and
// import is here to clearly separate changed aspects of the iface for Ohua
use smoltcp::iface::{IpPacket, Context as InterfaceInner};


pub type Messages = Vec<(SocketHandle, Vec<u8>)>;

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum InterfaceCall{
    InitPoll,
    InitIngress,
    // This represents the received data, the receive result and the tx token but probably there is a nicer way to do this.
    ProcessIngress(Option<(Vec<u8>, Result<()>, Option<()>)>),
    LoopIngress(Result<()>),
    InitEgress,
    PollLoopCondition,
    // To avoid the hassle with dyn Device::Token we use a simple ok for
    //  now to represent the token
    InnerDispatchLocal(Option<()>),
    // takes the DeviceResult
    MatchSocketDispatchAfter(Result<()>),
    // takes the emit result of the current socket
    HandleResult(Result<()>),
    // iterates to nex socket & packet
    UpdateEgressState,
    AnswerToSocket(Vec<(SocketHandle, Vec<u8>)>),
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum InterfaceState{
    Egress,
    Ingress
}


pub struct OInterface<'a> {
    inner: InterfaceInner<'a>,
    sockets:Option<SocketSet<'a>>,
    // We will probably replace this by a
    // State = Option<Ingress{}> | Option<Egress{}>
    current_egress_state: Option<EgressState<'a>>,
    current_ingress_state: Option<IngressState<'a>>,
    emitted_any:bool,
    processed_any:bool,
    readiness_changed:bool
}


struct EgressState<'es>{
    // We keep the interfaces sockets here during egress to mimic
    // the situation that the socket reference would be borrowed during egress
    // and becomes usable in  poll afterward
    sockets_during_egress: Option<SocketSet<'es>>,
    // We need to reuse it, doesn't live as long as the other refs
    current_handle:Option<usize>,
    // we need it just once at the end of the EgressStates lifetime
    current_neighbor:Option<IpAddress>,
    current_presend_packet: Option<IpPacketOwned>,
    //Copy of the intermediate results, we need to take it out and pass it to
    //dispatch_after
    current_postsend_packet:Option< (TcpReprP, IpRepr, bool)>
}

struct IngressState<'es>{
    // We keep the interfaces sockets here during egress to mimic
    // the situation that the socket reference would be borrowed during egress
    // and becomes usable in  poll afterward
    sockets_during_ingress: Option<SocketSet<'es>>,
}

#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) enum IpPacketOwned {
    Tcp((IpRepr, TcpReprP)),
}


impl  IpPacketOwned {
    pub(crate) fn to_ip_packet(&self) -> IpPacket<'_> {
        match self {
            IpPacketOwned::Tcp((ip_repr, tcp_repr_p)) =>
                IpPacket::Tcp((ip_repr.clone(), tcp_repr_p.to()))
        }
    }
    pub(crate) fn ip_repr(&self) -> IpRepr {
        match self {
            IpPacketOwned::Tcp((ip_repr, _tcp_repr)) => ip_repr.clone()
        }
    }
}
