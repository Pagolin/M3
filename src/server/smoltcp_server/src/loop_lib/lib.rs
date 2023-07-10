use m3::col::{String};

pub fn tinyfunction(input: &str) -> String {
    let numstring = String::from("Input was: ");
    //let owned = String::from(input);
    numstring + input
}
