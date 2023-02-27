/// Adapted abstraction of the device interface from smoltcp.
/// Methods to send and receive without passing tokens are implemented to support
/// distributed execution, i.e. IP stack and device interacting without shared references

use smoltcp::phy::{RxToken, TxToken, DeviceCapabilities};
use smoltcp::{Result, Error};
use smoltcp::time::{Duration, Instant};

use super::util::Either;
use super::interface::{InterfaceState, InterfaceCall};

use m3::vec::Vec;
use base::vec;

pub trait Device<'a> {
    type RxToken: RxToken + 'a;
    type TxToken: TxToken + 'a;

    /// Construct a token pair consisting of one receive token and one transmit token.
    ///
    /// The additional transmit token makes it possible to generate a reply packet based
    /// on the contents of the received packet. For example, this makes it possible to
    /// handle arbitrarily large ICMP echo ("ping") requests, where the all received bytes
    /// need to be sent back, without heap allocation.
    fn receive(&'a mut self) -> Option<(Self::RxToken, Self::TxToken)>;

    /// Construct a transmit token.
    fn transmit(&'a mut self) -> Option<Self::TxToken>;

    /// Get a description of device capabilities.
    fn capabilities(&self) -> DeviceCapabilities;

    fn send_tokenfree(&'a mut self, timestamp:Instant, packet:Vec<u8>) -> Result<()> {
        let sending_result = self
            .transmit()
            .ok_or_else(|| Error::Exhausted)
            .and_then(|token|
                   token.consume(timestamp, packet.len(),
                     |buffer| Ok(buffer.copy_from_slice(packet.as_slice()))));
        sending_result
    }

        /// To simplify things a bit we do not send tokens but merely the info
    fn receive_tokenfree(&'a mut self, timestamp: Instant
    ) -> Option<(Vec<u8>, Result<()>, Option<()>)> {
        if let Some((rx, _tx)) = self.receive() {
            let mut received_frame = vec![];
            let receiving_result = rx.consume(timestamp, |frame| { received_frame.extend_from_slice(frame); Ok(())});
            return Some((received_frame, receiving_result, Some(())))
        } else {
            None
        }
    }

    fn transmit_tokenfree(&'a mut self) -> Option<()> {
        if self.transmit().is_some() {
            Some(())
        } else {
            None
        }

    }

    /// Thi function returns true when the device is ready to be used
    /// in poll again. It resembles the implementation in M3
    fn needs_poll(&self, max_duration:Option<Duration>) -> bool;

    // ToDo: To keep it simple we currently just send around a simple Ok
    //       instead of a token. Can this lead to requesting from one device,
    //       while sending with another? (Not in pur code but in general)
    fn process_call(
        &'a mut self,
        dev_call_state:DeviceCall
    ) -> Either<InterfaceCall, (Option<Duration>, bool)>
    {
        match dev_call_state {
            DeviceCall::Transmit
                => Either::Left(InterfaceCall::InnerDispatchLocal(self.transmit_tokenfree())),
            DeviceCall::Consume(timestamp,packet, InterfaceState::Egress)
                => Either::Left(InterfaceCall::MatchSocketDispatchAfter(self.send_tokenfree(timestamp, packet))),
            DeviceCall::Consume(timestamp,packet, InterfaceState::Ingress)
                => Either::Left(InterfaceCall::LoopIngress(self.send_tokenfree(timestamp, packet))),
            DeviceCall::Receive(timestamp)
                => Either::Left(InterfaceCall::ProcessIngress(self.receive_tokenfree(timestamp))),
            DeviceCall::NeedsPoll(socket_wait_duration)
            // This is actually pretty clumsy, we do not want the interface to
            // process the waiting but we need to return an interface call here
                => Either::Right((socket_wait_duration, self.needs_poll(socket_wait_duration))),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum DeviceCall{
    Transmit,
    Consume(Instant, Vec<u8>, InterfaceState),
    Receive(Instant),
    NeedsPoll(Option<Duration>),
}
