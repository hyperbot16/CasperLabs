#![no_std]
#![feature(cell_update)]

extern crate alloc;

extern crate contract_ffi;

use contract_ffi::contract_api::{get_arg, revert, transfer_to_account, Error};
use contract_ffi::value::account::PublicKey;
use contract_ffi::value::U512;

#[no_mangle]
pub extern "C" fn call() {
    let amount: U512 = match get_arg(0) {
        Some(Ok(data)) => data,
        Some(Err(_)) => revert(Error::InvalidArgument),
        None => revert(Error::MissingArgument),
    };

    let public_key = PublicKey::new([42; 32]);
    let result = transfer_to_account(public_key, amount);
    assert_eq!(result, Err(Error::Transfer))
}
