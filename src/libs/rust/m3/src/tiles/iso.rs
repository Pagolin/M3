use crate::errors::Error;
use crate::cap::Selector;
use crate::tiles;
use crate::serialize::{M3Serializer, VecSink};
use crate::serialize::M3Deserializer;


pub trait Activatable {
    fn activate_from_selector(sel: Selector) -> Result<Self, Error> where Self: Sized;
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

    pub fn new_sink(&mut self) -> M3Serializer<VecSink<'_>> {
        self.act.data_sink()
    }
}

// I keep this function to make the trait dependency explicit.
pub fn sink<T: Capable>(sink: &mut M3Serializer<VecSink<'_>>, t: &T) {
    sink.push(t.sel())
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
        T::activate_from_selector(sel)
    }
}

#[macro_export]
macro_rules! activity {
    (| $($chans:ident : $types:ty),+ | $b:block ( $($def_chans:ident),+ ) ) => {
        {
            use $crate::tiles::iso;

            let mut act = iso::ChildActivity::new()?;
            $( act.delegate_cap(&$def_chans)?; )+
            let mut sink = act.new_sink();
            $( iso::sink(&mut sink, &$def_chans); )+
       
            act.act.run(|| {   
                let f = || -> Result<(), Error> {
                    let mut source = iso::OwnActivity::new();
                    $( let $chans : $types = source.activate()?; )+
                    $b
                };
                f().map(|_| 0).unwrap() // currently necessary because of the API
            })   
        }
    };
}
