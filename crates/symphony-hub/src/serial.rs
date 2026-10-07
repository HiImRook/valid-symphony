use std::ffi::c_void;
use std::io;
use std::ptr;

use crate::radar::Port;

type Handle = *mut c_void;

const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;
const OPEN_EXISTING: u32 = 3;
const INVALID_HANDLE_VALUE: isize = -1;
const PURGE_TXCLEAR: u32 = 0x0004;
const PURGE_RXCLEAR: u32 = 0x0008;
const MAXDWORD: u32 = 0xFFFF_FFFF;
const READ_WAIT_MS: u32 = 50;
const WRITE_TIMEOUT_MS: u32 = 2000;
const DCB_BINARY: u32 = 0x0001;
const DCB_DTR_ENABLE: u32 = 0x0010;
const DCB_RTS_ENABLE: u32 = 0x1000;
const DATA_BITS: u8 = 8;
const NO_PARITY: u8 = 0;
const ONE_STOP_BIT: u8 = 0;
const DEVICE_PREFIX: &str = r"\\.\";

#[repr(C)]
#[derive(Default)]
struct Dcb {
    dcb_length: u32,
    baud_rate: u32,
    flags: u32,
    w_reserved: u16,
    xon_lim: u16,
    xoff_lim: u16,
    byte_size: u8,
    parity: u8,
    stop_bits: u8,
    xon_char: i8,
    xoff_char: i8,
    error_char: i8,
    eof_char: i8,
    evt_char: i8,
    w_reserved1: u16,
}

#[repr(C)]
struct CommTimeouts {
    read_interval: u32,
    read_total_multiplier: u32,
    read_total_constant: u32,
    write_total_multiplier: u32,
    write_total_constant: u32,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateFileW(name: *const u16, access: u32, share: u32, security: *mut c_void, disposition: u32, flags: u32, template: Handle) -> Handle;
    fn CloseHandle(handle: Handle) -> i32;
    fn ReadFile(handle: Handle, buf: *mut c_void, len: u32, read: *mut u32, overlapped: *mut c_void) -> i32;
    fn WriteFile(handle: Handle, buf: *const c_void, len: u32, written: *mut u32, overlapped: *mut c_void) -> i32;
    fn GetCommState(handle: Handle, dcb: *mut Dcb) -> i32;
    fn SetCommState(handle: Handle, dcb: *const Dcb) -> i32;
    fn SetCommTimeouts(handle: Handle, timeouts: *const CommTimeouts) -> i32;
    fn PurgeComm(handle: Handle, flags: u32) -> i32;
}

pub struct SerialPort {
    handle: Handle,
}

impl SerialPort {
    pub fn open(name: &str, baud: u32) -> io::Result<Self> {
        let path: Vec<u16> = format!("{DEVICE_PREFIX}{name}").encode_utf16().chain(Some(0)).collect();
        let handle = unsafe { CreateFileW(path.as_ptr(), GENERIC_READ | GENERIC_WRITE, 0, ptr::null_mut(), OPEN_EXISTING, 0, ptr::null_mut()) };
        if handle as isize == INVALID_HANDLE_VALUE || handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mut port = SerialPort { handle };
        port.set_baud(baud)?;
        let timeouts = CommTimeouts {
            read_interval: MAXDWORD,
            read_total_multiplier: MAXDWORD,
            read_total_constant: READ_WAIT_MS,
            write_total_multiplier: 0,
            write_total_constant: WRITE_TIMEOUT_MS,
        };
        if unsafe { SetCommTimeouts(port.handle, &timeouts) } == 0 {
            return Err(io::Error::last_os_error());
        }
        port.clear_input()?;
        Ok(port)
    }
}

impl Port for SerialPort {
    fn write_all(&mut self, mut bytes: &[u8]) -> io::Result<()> {
        while !bytes.is_empty() {
            let mut written = 0u32;
            let ok = unsafe { WriteFile(self.handle, bytes.as_ptr() as *const c_void, bytes.len() as u32, &mut written, ptr::null_mut()) };
            if ok == 0 {
                return Err(io::Error::last_os_error());
            }
            if written == 0 {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "serial write timed out"));
            }
            bytes = &bytes[written as usize..];
        }
        Ok(())
    }

    fn read_some(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut read = 0u32;
        let ok = unsafe { ReadFile(self.handle, buf.as_mut_ptr() as *mut c_void, buf.len() as u32, &mut read, ptr::null_mut()) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(read as usize)
    }

    fn set_baud(&mut self, baud: u32) -> io::Result<()> {
        let mut dcb = Dcb { dcb_length: std::mem::size_of::<Dcb>() as u32, ..Default::default() };
        if unsafe { GetCommState(self.handle, &mut dcb) } == 0 {
            return Err(io::Error::last_os_error());
        }
        dcb.baud_rate = baud;
        dcb.flags = DCB_BINARY | DCB_DTR_ENABLE | DCB_RTS_ENABLE;
        dcb.byte_size = DATA_BITS;
        dcb.parity = NO_PARITY;
        dcb.stop_bits = ONE_STOP_BIT;
        if unsafe { SetCommState(self.handle, &dcb) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn clear_input(&mut self) -> io::Result<()> {
        if unsafe { PurgeComm(self.handle, PURGE_RXCLEAR | PURGE_TXCLEAR) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

impl Drop for SerialPort {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}
