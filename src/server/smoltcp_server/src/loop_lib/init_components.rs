use m3::col::{String, BTreeMap};
use m3::{log, vec};

use local_smoltcp::phy::{Device, DeviceCapabilities, Medium};
use local_smoltcp::iface::{Interface, InterfaceBuilder, NeighborCache, Messages};
use local_smoltcp::socket::{tcp};
use local_smoltcp::wire::{EthernetAddress, IpAddress, IpCidr};
use local_smoltcp::{Result};

use crate::loop_lib::store::{Store, Answer};

use crate::driver::*;

#[cfg(target_vendor = "gem5")]
pub type DeviceType = E1000Device;
#[cfg(target_vendor = "hw")]
pub type DeviceType = AXIEthDevice;
#[cfg(target_vendor = "host")]
pub type DeviceType = DevFifo;



#[cfg(target_vendor = "gem5")]
pub fn init_device() -> (E1000Device, DeviceCapabilities)
{

    let mut device = E1000Device::new().unwrap();
    let caps = device.capabilities();
    (device, caps)
}

#[cfg(target_vendor = "hw")]
pub fn init_device() -> (AXIEthDevice, DeviceCapabilities)
{
    let mut device = AXIEthDevice::new().unwrap();
    let caps = device.capabilities();
    (device, caps)
}

#[cfg(target_vendor = "host")]
pub fn init_device() -> (DevFifo, DeviceCapabilities)
{
    // The name parameter is used to identify the socket and is usually ser
    // via the app config e.g. in boot/rust-net-tests.xml
    let mut device = DevFifo::new("kvsocket");

    let caps = device.capabilities();
    (device, caps)
}

pub fn init_ip_stack(caps: DeviceCapabilities) -> Interface<'static> {
    let tcp_rx_buffer = tcp::SocketBuffer::new(vec![0; 1024]);
    let tcp_tx_buffer = tcp::SocketBuffer::new(vec![0; 2048]);
    let tcp_socket = tcp::Socket::new(tcp_rx_buffer, tcp_tx_buffer);

    let mut sockets = vec![];

    let neighbor_cache = NeighborCache::new(BTreeMap::new());
    let ethernet_addr = EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]);
    let ip_addrs = [
        IpCidr::new(IpAddress::v4(192, 168, 69, 2), 24)
    ];


    let medium = caps.medium;
    let mut builder = InterfaceBuilder::new(sockets).ip_addrs(ip_addrs);

    if medium == Medium::Ethernet {
        builder = builder.hardware_addr(ethernet_addr.into()).neighbor_cache(neighbor_cache);
    }

    let mut iface = builder.finalize_no_dev::<DeviceType>(caps);

    let tcp_handle = iface.add_socket(tcp_socket);
    iface
}

// The original application only had one socket (and one client in our test scenario)
// so we do not need socket or connection identifiers. If we do in future,
// We can augment the app to hold a list of socket identifiers to communicate with.

pub fn init_app() -> App {
    let store = Store::new("kvstore");
    App{ store }
}


pub struct App {
    store: Store,
    // tcp_socket_handles: Vec<SocketHandle>,
}


impl App {

    pub fn process_message(
        &mut self,
        poll_res: Result<bool>,
        mut messages:Messages
    ) -> Messages {
        match poll_res {
                Ok(_) => {}
                Err(e) => {
                    log!(true, "poll error: {}", e);
                }
            }
        for (handle, optn_msg) in messages.iter_mut() {
           if let Some(msg) = optn_msg {
               let answer = match self.store.handle_message(&msg){
                   Answer::Message(outbytes) => Some(outbytes),
                   // Client has sent "ENDNOW" so we need to stop to shutdown gracefully
                   Answer::Stop => {
                       log!(true, "Client sent ENDNOW, so Server will stop");
                       // We need to forward this to the interface, which is a bit of
                       // redundant data flow, but the packet parsing happens in the store ¯\_(ツ)_/¯
                       Some(b"ENDNOW".to_vec())
                   },
                   // There wasn't enough data for a complete request -> we'll get more from the client in the next poll
                   Answer::Nothing => None,
                };
               *optn_msg = answer;
            }
        }
        messages
    }
}
