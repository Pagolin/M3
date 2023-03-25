use m3::col::{String, Vec, ToString};
use m3::{vec, log, println};
use m3::serde::{Deserialize, Serialize};
use m3::serialize::M3Deserializer;
use core::{str, array};
use core::convert::TryFrom;

const USIZE_LENGTH:usize = 8;

#[derive(Clone, Default)]
pub struct Store {
    // ToDo: Replace <data> with a handle to LevelDB
    data: Vec<String>,
    unfinished_operation : Vec<u8>,
}

impl Store {
    //pub fn default() -> Store {
      //  unimplemented!()
    //}

    pub fn handle_message(&mut self, input_bytes:&[u8]) -> Option<Vec<u8>>{
        /*
        So we get a new message. The Data Stream will look something like this:
        ... 10 <10 bytes Operation> 23 <23 bytes Operation> 14 <14 bytes Operation> ...
        so a number indicating the length of the coming operation. The socket
        will give us as much bytes, as it can receive at once. So we must expect any
        kind of chunks. So we will store
        a) how many bytes we currently expect (if we need to
        complete an operation) and
        b) bytes of an incomplete operation

        The 'original' lvldb server would try to receive until it as a complete operation.
        The (roughly) equivalent behaviour in our scenario is to return no answer and wait
        for the next input.
        */
        let mut input_bytes_vec = input_bytes.to_vec();

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
                // For now we do nothing, later this should be an error
                println!("There was no length. We tell the client to stop");
                return Some(b"ERROR".to_vec())
            }
            // succeeds => we now how many bytes we need for the next operation
            Some(l) => {
                op_len = l;
                // The first 4 bytes where the length, we store them in case we can't finish
                // the operation yet
                length_bytes = operation_bytes;
                operation_bytes = length_bytes.split_off(USIZE_LENGTH);
                println!("Expected operation length is {} and we have {} operation bytes", l, operation_bytes.len());
            }

        }


        // try to get 'remaining_op_len' bytes from the rest of the packet
        if operation_bytes.len() < op_len {
            // We will not get the whole operation from this packet
            // so we store the lenght bytes and the operation bytes and
            // start over next time

            self.unfinished_operation.append(&mut length_bytes);
            self.unfinished_operation.append(&mut operation_bytes);
            println!("To few bytes for operation. We stored {:?} for later", str::from_utf8(&self.unfinished_operation));
            // We're done until te next packet arrives
            return None
        } else {
            let mut remainder = operation_bytes.split_off(op_len);
            self.unfinished_operation = remainder;
            println!("Sufficient bytes for operation.\
                     We process {:?},\n and store {:?} for later"
                    , str::from_utf8(&operation_bytes)
                     , str::from_utf8(&self.unfinished_operation));
            let answer = self.answer(operation_bytes);
            Some(answer)
        }


    }


    fn answer(&mut self, operation_bytes: Vec<u8>) -> Vec<u8>{
	// ToDo: This used to be where we deserializa and ask the HashMap
	//       Now we need to replace this code with a call to LevelDB
        return operation_bytes
    }

    fn get_operation_len(&self, input_bytes:& [u8]) -> Option<usize> {
        // We assume length to be u32, so we need at least 4 u8 in the input to be a valid length
        if input_bytes.len() < USIZE_LENGTH {
            return None
        }
        let (len_bytes, _rest) = input_bytes.split_at(USIZE_LENGTH);
        let new_len = usize::from_be_bytes(
            <[u8;USIZE_LENGTH]>::try_from(len_bytes).expect("Failed to convert length byte array"));
        Some(new_len)
    }
}