use m3::vec::Vec;

use smoltcp::wire::{TcpRepr, TcpControl, TcpSeqNumber, tcpfield as field};

#[derive(Debug, PartialEq)]
pub struct ReprOwned {
    pub src_port: u16,
    pub dst_port: u16,
    pub control: TcpControl,
    pub seq_number: TcpSeqNumber,
    pub ack_number: Option<TcpSeqNumber>,
    pub window_len: u16,
    pub window_scale: Option<u8>,
    pub max_seg_size: Option<u16>,
    pub sack_permitted: bool,
    pub sack_ranges: [Option<(u32, u32)>; 3],
    pub payload: Vec<u8>,
    pub payload_len: usize
}


impl ReprOwned {
    pub fn from(tcp_repr: TcpRepr) -> Self {
        ReprOwned {
            src_port : tcp_repr.src_port,
            dst_port : tcp_repr.dst_port,
            control : tcp_repr.control,
            seq_number : tcp_repr.seq_number,
            ack_number : tcp_repr.ack_number,
            window_len : tcp_repr.window_len,
            window_scale : tcp_repr.window_scale,
            max_seg_size : tcp_repr.max_seg_size,
            sack_permitted : tcp_repr.sack_permitted,
            sack_ranges : tcp_repr.sack_ranges,
            payload: tcp_repr.payload.to_vec(),
            payload_len : tcp_repr.payload.len()
        }
    }
    pub fn to(&self) -> TcpRepr {
        TcpRepr{
            src_port: self.src_port,
            dst_port: self.dst_port,
            control: self.control,
            seq_number: self.seq_number,
            ack_number: self.ack_number,
            window_len: self.window_len,
            window_scale: self.window_scale,
            max_seg_size: self.max_seg_size,
            sack_permitted: self.sack_permitted,
            sack_ranges: self.sack_ranges,
            payload: &*self.payload
        }
    }

    pub fn segment_len(&self) -> usize {
        self.payload_len + self.control.len()
    }

    pub fn header_len(&self) -> usize {
        let mut length = field::URGENT.end;
        if self.max_seg_size.is_some() {
            length += 4
        }
        if self.window_scale.is_some() {
            length += 3
        }
        if self.sack_permitted {
            length += 2;
        }
        let sack_range_len: usize = self
            .sack_ranges
            .iter()
            .map(|o| o.map(|_| 8).unwrap_or(0))
            .sum();
        if sack_range_len > 0 {
            length += sack_range_len + 2;
        }
        if length % 4 != 0 {
            length += 4 - length % 4;
        }
        length
    }

    pub fn buffer_len(&self) -> usize {
        self.header_len() + self.payload_len
    }
}

