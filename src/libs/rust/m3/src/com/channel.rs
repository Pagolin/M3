use crate::cap::Selector;
use crate::errors::Error;
use crate::com::{RecvGate, SendGate, SGateArgs};
use crate::com::stream::{recv_msg};
use crate::serialize::{Serialize, Deserialize};
use crate::math;
use crate::tiles::iso::{Capable, Activatable};

pub struct Sender {
    sgate: SendGate
}

pub struct Receiver {
    rgate: RecvGate
}

impl Sender{
    fn new(rgate: &RecvGate, credits: u32) -> Result<Self, Error> {
        let sgate = SendGate::new_with(SGateArgs::new(rgate).credits(credits))?;
        Ok(Sender { sgate })
    }

    pub fn send<T: Serialize>(&self, data: T) -> Result<(), Error> {
        send_vmsg!(&self.sgate, RecvGate::def(), data)
    }
}

impl Activatable for Sender {
    fn activate(sel: Selector) -> Result<Self, Error> {
        let sgate = SendGate::new_bind(sel);
        sgate.activate()?;
        Ok(Sender { sgate })
    }
}

impl Capable for Sender {
    fn sel(&self) -> Selector {
        self.sgate.sel()
    }
}


impl Receiver {
    pub fn new(order: usize, msg_order: usize) -> Result<Self, Error> {
        let rgate = RecvGate::new(math::next_log2(order), math::next_log2(msg_order))?; 
        Ok(Receiver { rgate } )
    }

    fn sender(&self, credits: u32) -> Result<Sender, Error> {
        Sender::new(&self.rgate, credits)
    }

    pub fn recv<T: Deserialize<'static>>(&self) -> Result<T,Error> {
        recv_msg(&self.rgate)?.pop::<T>()
    }
}

impl Activatable for Receiver {
    fn activate(sel: Selector) -> Result<Self, Error> {
        let mut rgate = RecvGate::new_bind(
            sel,
            math::next_log2(256), 
            math::next_log2(256)); 
        rgate.activate()?;
        Ok(Receiver { rgate } )
    }
}

impl Capable for Receiver {
    fn sel(&self) -> Selector {
        self.rgate.sel()
    }
}

pub fn channel(order: usize, msg_order: usize, credits: u32) -> Result<(Sender, Receiver), Error> {
    let rx = Receiver::new(order, msg_order)?;
    let tx = rx.sender(credits)?;
    Ok((tx, rx))
}

pub fn channel_def() -> Result<(Sender, Receiver), Error> {
    channel(256, 256, 1)
}



