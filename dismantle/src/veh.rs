use windows::Win32::Foundation::EXCEPTION_CONTINUE_EXECUTION;
use windows::Win32::Foundation::EXCEPTION_CONTINUE_SEARCH;
use windows::Win32::System::Diagnostics::Debug::{
    EXCEPTION_POINTERS, EXCEPTION_SINGLE_STEP, CONTEXT_DEBUG_REGISTERS,
};
use windows::Win32::System::Memory::{VirtualProtect, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS, PAGE_GUARD};
use windows::Win32::System::Threading::{GetCurrentProcess, TerminateProcess};
use iced_x86::{Decoder, DecoderOptions};
use std::sync::Mutex;
use indexmap::IndexMap;
use crate::utils::{log_winapi_fail, log_program_fail};

// Exception codes
pub const STATUS_GUARD_PAGE_VIOLATION: u32 = 0x80000001;
pub const PAGE_GUARD_EXECUTE_EXCEPTION: usize = 8;

// Constants
pub const ASM_INT3: u8 = 0xcc;
pub const ASM_MAX_INSTRUCTION_LEN_X64: usize = 16;

// Global state
static PAYLOAD_EXECUTED_ASM: Mutex<Option<*mut IndexMap<u32, Vec<u8>>>> = Mutex::new(None);
static PAYLOAD_BASE: Mutex<Option<*mut u8>> = Mutex::new(None);
static PAYLOAD_SIZE: Mutex<Option<u32>> = Mutex::new(None);

/// Initialize global state for VEH
pub unsafe fn init_veh_globals(
    asm_map: *mut IndexMap<u32, Vec<u8>>,
    base: *mut u8,
    size: u32,
) {
    *PAYLOAD_EXECUTED_ASM.lock().unwrap() = Some(asm_map);
    *PAYLOAD_BASE.lock().unwrap() = Some(base);
    *PAYLOAD_SIZE.lock().unwrap() = Some(size);
}

/// Calculate offset from payload base address
#[cfg(target_arch = "x86_64")]
unsafe fn find_rip_offset_from_payload_base(rip: u64) -> u32 {
    let base = (*PAYLOAD_BASE.lock().unwrap()).unwrap() as u64;
    (rip - base) as u32
}

#[cfg(target_arch = "x86")]
unsafe fn find_rip_offset_from_payload_base(eip: u32) -> u32 {
    let base = (*PAYLOAD_BASE.lock().unwrap()).unwrap() as u32;
    eip - base
}

/// Vectored Exception Handler for PAGE_GUARD protection
/// This handler records executed instructions
pub unsafe extern "system" fn veh_page_guard(exception_info: *mut EXCEPTION_POINTERS) -> i32 {
    let exception_record = (*exception_info).ExceptionRecord;
    let context = (*exception_info).ContextRecord;

    let exception_code = (*exception_record).ExceptionCode.0;

    // Handle PAGE_GUARD violations
    if exception_code == STATUS_GUARD_PAGE_VIOLATION {
        let exception_info_type = (*exception_record).ExceptionInformation[0];

        // Check if this is an execute exception
        if exception_info_type == PAGE_GUARD_EXECUTE_EXCEPTION {
            #[cfg(target_arch = "x86_64")]
            let instruction_pointer = (*context).Rip;

            #[cfg(target_arch = "x86")]
            let instruction_pointer = (*context).Eip as u64;

            let asm_offset = find_rip_offset_from_payload_base(instruction_pointer);

            // Get the map reference
            let map_ptr = (*PAYLOAD_EXECUTED_ASM.lock().unwrap()).unwrap();
            let payload_executed_asm = &mut *map_ptr;

            // If this offset hasn't been recorded yet
            if !payload_executed_asm.contains_key(&asm_offset) {
                // Decode the instruction at this address
                let code_slice = std::slice::from_raw_parts(
                    instruction_pointer as *const u8,
                    15,
                );

                #[cfg(target_arch = "x86_64")]
                let mut decoder = Decoder::with_ip(64, code_slice, instruction_pointer, DecoderOptions::NONE);

                #[cfg(target_arch = "x86")]
                let mut decoder = Decoder::with_ip(32, code_slice, instruction_pointer as u64, DecoderOptions::NONE);

                if let Some(instruction) = decoder.iter().next() {
                    let instruction_len = instruction.len();

                    // Copy the instruction bytes
                    let instruction_bytes = std::slice::from_raw_parts(
                        instruction_pointer as *const u8,
                        instruction_len,
                    ).to_vec();

                    // Record this instruction
                    payload_executed_asm.insert(asm_offset, instruction_bytes);
                }
            }
        }

        // Set trap flag to trigger EXCEPTION_SINGLE_STEP
        (*context).EFlags |= 0x100;

        return EXCEPTION_CONTINUE_EXECUTION.0;
    }
    // Handle single-step exceptions to restore PAGE_GUARD
    else if exception_code == EXCEPTION_SINGLE_STEP.0 {
        let base = (*PAYLOAD_BASE.lock().unwrap()).unwrap();
        let size = (*PAYLOAD_SIZE.lock().unwrap()).unwrap();

        let mut old_protect = PAGE_PROTECTION_FLAGS(0);
        let result = VirtualProtect(
            base as *const _,
            size as usize,
            PAGE_EXECUTE_READWRITE | PAGE_GUARD,
            &mut old_protect,
        );

        if result.is_err() {
            log_winapi_fail("VirtualProtect");
            log_program_fail("Failed to restore page guard protections in VEH... Program will now terminate...");
            let _ = TerminateProcess(GetCurrentProcess(), 1);
        }

        return EXCEPTION_CONTINUE_EXECUTION.0;
    }

    EXCEPTION_CONTINUE_SEARCH.0
}
