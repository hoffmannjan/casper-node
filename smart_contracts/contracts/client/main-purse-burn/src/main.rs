#![no_std]
#![no_main]

extern crate alloc;
use alloc::{string::String, vec::Vec};

use casper_contract::{
    contract_api::{account, alloc_bytes, runtime, system},
    ext_ffi,
    unwrap_or_revert::UnwrapOrRevert,
};
use casper_types::{
    api_error, bytesrepr, runtime_args, system::mint, ApiError, Key, RuntimeArgs, URef, U512,
};

const ARG_PURSE_NAME: &str = "purse_name";

fn burn(uref: URef, amount: U512) {
    let contract_hash = system::get_mint();
    let args = runtime_args! {
        mint::ARG_PURSE => uref,
        mint::ARG_AMOUNT => amount,
    };
    runtime::call_contract::<()>(contract_hash, mint::METHOD_BURN, args);
}

#[no_mangle]
pub extern "C" fn call() {
    let purse_uref = match get_named_arg_if_exists::<String>(ARG_PURSE_NAME) {
        Some(name) => {
            // if a key was provided and there is no value under it we revert
            // to prevent user from accidentaly burning tokens from the main purse
            // eg. if they make a typo
            let Some(Key::URef(purse_uref)) = runtime::get_key(&name) else {
                runtime::revert(ApiError::InvalidPurseName)
            };
            purse_uref
        }
        None => account::get_main_purse(),
    };
    let amount: U512 = runtime::get_named_arg(mint::ARG_AMOUNT);

    burn(purse_uref, amount);
}

fn get_named_arg_if_exists<T: bytesrepr::FromBytes>(name: &str) -> Option<T> {
    let arg_size = {
        let mut arg_size: usize = 0;
        let ret = unsafe {
            ext_ffi::casper_get_named_arg_size(
                name.as_bytes().as_ptr(),
                name.len(),
                &mut arg_size as *mut usize,
            )
        };
        match api_error::result_from(ret) {
            Ok(_) => Some(arg_size),
            Err(ApiError::MissingArgument) => None,
            Err(e) => runtime::revert(e),
        }
    }?;
    let arg_bytes = if arg_size > 0 {
        let res = {
            let data_non_null_ptr = alloc_bytes(arg_size);
            let ret = unsafe {
                ext_ffi::casper_get_named_arg(
                    name.as_bytes().as_ptr(),
                    name.len(),
                    data_non_null_ptr.as_ptr(),
                    arg_size,
                )
            };
            let data =
                unsafe { Vec::from_raw_parts(data_non_null_ptr.as_ptr(), arg_size, arg_size) };
            api_error::result_from(ret).map(|_| data)
        };
        // Assumed to be safe as `get_named_arg_size` checks the argument already
        res.unwrap_or_revert()
    } else {
        // Avoids allocation with 0 bytes and a call to get_named_arg
        Vec::new()
    };
    let value = bytesrepr::deserialize(arg_bytes).unwrap_or_revert_with(ApiError::InvalidArgument);
    Some(value)
}
