use m3::col::Vec;
use m3::{vec, log, println};
use core::{str};
use core::convert::TryFrom;

const USIZE_LENGTH:usize = 8;
const MAX_PLAUSIBLE_PACKAGE_LENGTH:usize = 3000;

opaque!{
    /// Opaque handle representing an opened database. The handle is thread-safe.
    pub struct leveldb_t;
}

#[repr(C)]
pub struct DBResult {
    pub db: *mut leveldb_t,
    pub success: bool,
}
extern "C" {
    // For now we'll use a default name defined in c++
    fn leveldb_open_wrapper() -> DBResult;
    fn leveldb_close(db: *mut leveldb_t);
    // FIXME: usize != size_t i.e. not necessarily the same so I should rather use libc::size_t
    fn execute(db: *mut leveldb_t, package_buffer: *const u8, package_size: usize) -> usize;
}



struct RawDB {
    ptr: *mut leveldb_t,
}

impl RawDB {
    fn new(_name:&str) -> Self {
        let DBResult{ db, success } = unsafe {leveldb_open_wrapper()};
        if !success {
            // Todo: End simulation upon failure
            println!("Creating db failed");
        } else {
            println!("DB creation successful")
        }
        RawDB { ptr :db}
    }
}

impl Drop for RawDB {
    fn drop(&mut self) {
        unsafe {
            leveldb_close(self.ptr);
        }
    }
}

pub enum Answer {
    Message(Vec<u8>),
    Nothing,
    Stop,
}

pub struct Store {
    // ToDo: Replace <data> with a handle to LevelDB
    data: RawDB,
    unfinished_operation : Vec<u8>,
}

impl Store {
    pub fn new(name: &str) -> Store {
        Store {
            data: RawDB::new(name),
            unfinished_operation: vec![],
        }
    }

    pub fn handle_message(&mut self, input_bytes:&[u8]) -> Answer {
        /*
        So we get a new message. The Data Stream will look something like this:
        ... 10 <10 bytes Operation> 23 <23 bytes Operation> 14 <14 bytes Operation> ...
        so a number indicating the length of the coming operation. The socket
        will give us as much bytes, as it can receive at once. So we must expect any
        kind of chunks. So we will store
        a) how many bytes we currently expect (if we need to
        complete an operation) and
        b) bytes of an incomplete operation

        The 'original' lvldb smoltcp_server would try to receive until it as a complete operation.
        The (roughly) equivalent behaviour in our scenario is to return no answer and wait
        for the next input.
        */
        let mut input_bytes_vec = input_bytes.to_vec();
        //println!("Store got {:?} new bytes", input_bytes_vec.len());
        if input_bytes_vec.len() + self.unfinished_operation.len() < USIZE_LENGTH  {
            //println!("Input was to short to contain a valid length info");
            self.unfinished_operation.append(&mut input_bytes_vec);
            return Answer::Nothing
        }

        let mut operation_bytes =vec![];
        let mut length_bytes = vec![];
        let mut op_len = 0;
	
        // In general we can append the new data to the 'unfinished operation' since it will
        // be empty if there's nothing unfinished. Doing so, has the following advantage: If
        // the operation length (currently the length of usize as bytes) is unfortunately split to the end of the last and
        // beginning of the new paket, we can reconstruct it this way.
        operation_bytes.append(&mut self.unfinished_operation);
        operation_bytes.append(&mut input_bytes_vec);


        let optn_new_len = self.get_operation_len(&operation_bytes);

        match optn_new_len {
            // fails => return error
            None => {
                println!("There was no length. We tell the client to stop");
                let mut error_msg = b"ERROR".to_vec();
                let mut header_and_msg = error_msg.len().to_be_bytes().to_vec();
                header_and_msg.append(&mut error_msg);
                return Answer::Message(header_and_msg)
            }
            // succeeds => we now how many bytes we need for the next operation
            Some(l) => {
                op_len = l;
                // The first 4 bytes where the length, we store them in case we can't finish
                // the operation yet
                length_bytes = operation_bytes;
                operation_bytes = length_bytes.split_off(USIZE_LENGTH);
            }

        }

        if op_len == 6 {
            if let Ok("ENDNOW") = str::from_utf8(&operation_bytes){
                println!("Should end");
                return Answer::Stop
            }
        }

        // try to get 'remaining_op_len' bytes from the rest of the packet
        if operation_bytes.len() < op_len {
            // We will not get the whole operation from this packet
            // so we store the length bytes and the operation bytes and
            // start over next time
            self.unfinished_operation.append(&mut length_bytes);
            self.unfinished_operation.append(&mut operation_bytes);
            // println!("To few bytes for operation. We stored {:?} bytes for later", self.unfinished_operation.len());
            // We're done until te next packet arrives
            return Answer::Nothing
        } else {
            let remainder = operation_bytes.split_off(op_len);
            self.unfinished_operation = remainder;
            /*println!("Sufficient bytes for operation.\
                     We process {:?} bytes ,\n and store {:?} bytes for later"
                    , operation_bytes.len()
                     , self.unfinished_operation.len());*/
            let answer = self.answer(operation_bytes);
            return Answer::Message(answer)
        }
    }


    fn answer(&mut self, mut operation_bytes: Vec<u8>) -> Vec<u8>{
        let answer_length = unsafe {
            execute(self.data.ptr, operation_bytes.as_mut_ptr(), operation_bytes.len())
        };
        let mut count_and_bytes = answer_length
            .to_be_bytes()
            .to_vec();
        let mut bytes = vec![0; answer_length];
        count_and_bytes.append(&mut bytes);
        return count_and_bytes
    }

    fn get_operation_len(&self, input_bytes:& [u8]) -> Option<usize> {
        // We assume length to be u32, so we need at least 4 u8 in the input to be a valid length
        if input_bytes.len() < USIZE_LENGTH {
            //println!("Input too small");
            return None
        }
        let (len_bytes, _rest) = input_bytes.split_at(USIZE_LENGTH);
        let new_len = usize::from_be_bytes(
            <[u8;USIZE_LENGTH]>::try_from(len_bytes).expect("Failed to convert length byte array"));
        // We need a plausibility check here as anything that can be interpreted as a usize will
        // be, even if it was a sequence of special characters
        if new_len > MAX_PLAUSIBLE_PACKAGE_LENGTH{
            //println!("Length conversion to usize failed");
            return None
        }
        Some(new_len)
    }
}