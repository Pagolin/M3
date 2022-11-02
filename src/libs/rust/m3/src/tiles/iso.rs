use crate::errors::Error;
use crate::cap::Selector;
use crate::tiles;
use crate::serialize::{M3Serializer, VecSink};
use crate::serialize::M3Deserializer;


pub trait Activatable {
    fn activate(sel: Selector) -> Result<Self, Error> where Self: Sized;
}

pub trait Capable {
    fn sel(&self) -> Selector;
}


pub struct ChildActivity {
    pub act: tiles::ChildActivity
}

impl ChildActivity {
    pub fn new() -> Result<Self, Error> {
            Ok( ChildActivity { 
                act: tiles::ChildActivity::new_with(
                        tiles::Tile::get("clone")?, 
                        tiles::ActivityArgs::new("1-1-Activity"))?, 
            } )
    }

    pub fn delegate_cap<T: Capable>(&mut self, t: &T) -> Result<(), Error>{
        self.act.delegate_obj(t.sel())
    }

    pub fn new_sink(&mut self) -> ChannelSink<'_> {
        ChannelSink {
            sink: self.act.data_sink()
        }
    }
}

pub struct ChannelSink<'a> {
    sink: M3Serializer<VecSink<'a>>
}

impl<'a> ChannelSink<'a> {
    pub fn sink<T: Capable>(&'a mut self, t: &T) {
        self.sink.push(t.sel())
    }
}

pub struct OwnActivity<'a> {
    reg: M3Deserializer<'a>
}

impl<'a> OwnActivity<'a> {
    pub fn new() -> Self {
        let reg = tiles::Activity::own().data_source();
        OwnActivity { reg }
    }

    pub fn activate<T:Activatable>(&mut self) -> Result<T, Error> {
        let sel = self.reg.pop()?;
        T::activate(sel)
    }
}

#[macro_export]
macro_rules! delegate_channel_caps {
    ($act:ident, $chan:ident) => {
        $act.delegate_cap(&$chan).unwrap(); // FIXME use ?
    };

    ($sink:ident, $chan:ident, $($chans:ident),+) => {
        {
            delegate_channel_caps!($sink, $chan);
            delegate_channel_caps!($sink, $($chans),+ );
        }
    };
}

#[macro_export]
macro_rules! sink_channels {
    ($sink:ident, $chan:ident) => {
        $sink.sink(&$chan);
    };

    ($sink:ident, $chan:ident, $($chans:ident),+) => {
        {
            sink_channels!($sink, $chan);
            sink_channels!($sink, $($chans),+ );
        }
    };
}

#[macro_export]
macro_rules! source_channels {
    ($source:ident, $chan:ident : $type:ty) => {
        let $chan:$type = $source.activate()?;
    };

    ($source:ident, $chan:ident : $type:ty, $($chans:ident : $types:ty),+) => {
        {
            source_channels!($source, $chan : $type);
            source_channels!($source, $($chans : $types),+ );
        }
    };
}

#[macro_export]
macro_rules! activity {
    (|$($chans:ident : $types:ty),+| $b:block ($($def_chans:ident),+) ) => {
        {
            use m3::{source_channels, sink_channels, delegate_channel_caps};
            use m3::tiles::iso;

            let act = iso::ChildActivity::new().unwrap();// FIXME use ?
            delegate_channel_caps!(act, $($def_chans),+);
            let mut sink = act.new_sink();
            sink_channels!(sink, $($def_chans),+ );
       
            act.act.run(|| {   
                let f = || -> Result<(), Error> {
                    let mut source = iso::OwnActivity::new();
                    source_channels!(source, $($chans : $types),+ );
                
                    $b
                };
                f().map(|_| 0).unwrap() // currently necessary because of the API
            })   
        }
    };
}
