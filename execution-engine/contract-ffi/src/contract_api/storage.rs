use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::convert::{From, TryFrom, TryInto};
use core::u8;

use super::alloc_util::{alloc_bytes, str_ref_to_ptr, to_ptr};
use crate::bytesrepr::{self, deserialize, ToBytes};
use crate::contract_api::{ContractRef, TURef};
use crate::ext_ffi;
use crate::key::{Key, UREF_SIZE};
use crate::uref::AccessRights;
use crate::value::{Contract, Value};

pub(crate) fn read_untyped(key: &Key) -> Result<Option<Value>, bytesrepr::Error> {
    // Note: _bytes is necessary to keep the Vec<u8> in scope. If _bytes is
    //      dropped then key_ptr becomes invalid.

    let (key_ptr, key_size, _bytes) = to_ptr(key);
    let value_size = unsafe { ext_ffi::read_value(key_ptr, key_size) };
    let value_ptr = alloc_bytes(value_size);
    let value_bytes = unsafe {
        ext_ffi::get_read(value_ptr);
        Vec::from_raw_parts(value_ptr, value_size, value_size)
    };
    deserialize(&value_bytes)
}

fn try_into<T>(maybe_value: Option<Value>) -> Result<Option<T>, bytesrepr::Error>
where
    T: TryFrom<Value>,
{
    match maybe_value {
        None => Ok(None),
        Some(value) => value
            .try_into()
            .map(Some)
            .map_err(|_| bytesrepr::Error::custom("T could not be derived from Value")),
    }
}

/// Read value under the key in the global state
pub fn read<T>(turef: TURef<T>) -> Result<Option<T>, bytesrepr::Error>
where
    T: TryFrom<Value>,
{
    let key: Key = turef.into();
    let maybe_value = read_untyped(&key)?;
    try_into(maybe_value)
}

/// Reads the value at the given key in the context-local partition of global
/// state
pub fn read_local<K, V>(key: K) -> Result<Option<V>, bytesrepr::Error>
where
    K: ToBytes,
    V: TryFrom<Value>,
{
    let key_bytes = key.to_bytes()?;
    let maybe_value = read_untyped_local(&key_bytes)?;
    try_into(maybe_value)
}

fn read_untyped_local(key_bytes: &[u8]) -> Result<Option<Value>, bytesrepr::Error> {
    let key_bytes_ptr = key_bytes.as_ptr();
    let key_bytes_size = key_bytes.len();
    let value_size = unsafe { ext_ffi::read_value_local(key_bytes_ptr, key_bytes_size) };
    let value_ptr = alloc_bytes(value_size);
    let value_bytes = unsafe {
        ext_ffi::get_read(value_ptr);
        Vec::from_raw_parts(value_ptr, value_size, value_size)
    };
    deserialize(&value_bytes)
}

/// Write the value under the key in the global state
pub fn write<T>(turef: TURef<T>, t: T)
where
    Value: From<T>,
{
    let key = turef.into();
    let value = t.into();
    write_untyped(&key, &value)
}

fn write_untyped(key: &Key, value: &Value) {
    let (key_ptr, key_size, _bytes) = to_ptr(key);
    let (value_ptr, value_size, _bytes2) = to_ptr(value);
    unsafe {
        ext_ffi::write(key_ptr, key_size, value_ptr, value_size);
    }
}

/// Writes the given value at the given key in the context-local partition of
/// global state
pub fn write_local<K, V>(key: K, value: V)
where
    K: ToBytes,
    V: Into<Value>,
{
    let key_bytes = key.to_bytes().unwrap();
    write_untyped_local(&key_bytes, &value.into());
}

fn write_untyped_local(key_bytes: &[u8], value: &Value) {
    let key_bytes_ptr = key_bytes.as_ptr();
    let key_bytes_size = key_bytes.len();
    let (value_ptr, value_size, _bytes2) = to_ptr(value);
    unsafe {
        ext_ffi::write_local(key_bytes_ptr, key_bytes_size, value_ptr, value_size);
    }
}

/// Add the given value to the one currently under the key in the global state
pub fn add<T>(turef: TURef<T>, t: T)
where
    Value: From<T>,
{
    let key = turef.into();
    let value = t.into();
    add_untyped(&key, &value)
}

fn add_untyped(key: &Key, value: &Value) {
    let (key_ptr, key_size, _bytes) = to_ptr(key);
    let (value_ptr, value_size, _bytes2) = to_ptr(value);
    unsafe {
        // Could panic if the value under the key cannot be added to
        // the given value in memory
        ext_ffi::add(key_ptr, key_size, value_ptr, value_size);
    }
}

/// Stores the serialized bytes of an exported function under a URef generated by the host.
pub fn store_function(name: &str, named_keys: BTreeMap<String, Key>) -> ContractRef {
    let (fn_ptr, fn_size, _bytes1) = str_ref_to_ptr(name);
    let (keys_ptr, keys_size, _bytes2) = to_ptr(&named_keys);
    let mut addr = [0u8; 32];
    unsafe {
        ext_ffi::store_function(fn_ptr, fn_size, keys_ptr, keys_size, addr.as_mut_ptr());
    }
    ContractRef::URef(TURef::<Contract>::new(addr, AccessRights::READ_ADD_WRITE))
}

/// Stores the serialized bytes of an exported function at an immutable address generated by the
/// host.
pub fn store_function_at_hash(name: &str, named_keys: BTreeMap<String, Key>) -> ContractRef {
    let (fn_ptr, fn_size, _bytes1) = str_ref_to_ptr(name);
    let (keys_ptr, keys_size, _bytes2) = to_ptr(&named_keys);
    let mut addr = [0u8; 32];
    unsafe {
        ext_ffi::store_function_at_hash(fn_ptr, fn_size, keys_ptr, keys_size, addr.as_mut_ptr());
    }
    ContractRef::Hash(addr)
}

/// Returns a new unforgable pointer, where value is initialized to `init`
pub fn new_turef<T>(init: T) -> TURef<T>
where
    Value: From<T>,
{
    let key_ptr = alloc_bytes(UREF_SIZE);
    let value: Value = init.into();
    let (value_ptr, value_size, _bytes2) = to_ptr(&value);
    let bytes = unsafe {
        ext_ffi::new_uref(key_ptr, value_ptr, value_size); // new_uref creates a URef with ReadWrite access writes
        Vec::from_raw_parts(key_ptr, UREF_SIZE, UREF_SIZE)
    };
    let key: Key = deserialize(&bytes).unwrap();
    if let Key::URef(uref) = key {
        TURef::from_uref(uref).unwrap()
    } else {
        panic!("URef FFI did not return a valid URef!");
    }
}
