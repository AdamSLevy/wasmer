use crate::{
    error::{update_last_error, CApiError},
    instance::wasmer_instance_context_t,
    instance::wasmer_instance_t,
    module::wasmer_module_t,
    wasmer_result_t,
};

use std::slice;

use wasmer_runtime_core::backend::Compiler;

/// Creates a new Module with gas limit from the given wasm bytes.
///
/// Returns `wasmer_result_t::WASMER_OK` upon success.
///
/// Returns `wasmer_result_t::WASMER_ERROR` upon failure. Use `wasmer_last_error_length`
/// and `wasmer_last_error_message` to get an error message.
#[allow(clippy::cast_ptr_alignment)]
#[no_mangle]
pub unsafe extern "C" fn wasmer_compile_with_gas_metering(
    module: *mut *mut wasmer_module_t,
    wasm_bytes: *mut u8,
    wasm_bytes_len: u32,
) -> wasmer_result_t {
    if module.is_null() {
        update_last_error(CApiError {
            msg: "module is null".to_string(),
        });
        return wasmer_result_t::WASMER_ERROR;
    }
    if wasm_bytes.is_null() {
        update_last_error(CApiError {
            msg: "wasm bytes is null".to_string(),
        });
        return wasmer_result_t::WASMER_ERROR;
    }

    let bytes: &[u8] = slice::from_raw_parts_mut(wasm_bytes, wasm_bytes_len as usize);
    let result = wasmer_runtime_core::compile_with(bytes, &get_metered_compiler());
    let new_module = match result {
        Ok(instance) => instance,
        Err(_) => {
            update_last_error(CApiError {
                msg: "compile error".to_string(),
            });
            return wasmer_result_t::WASMER_ERROR;
        }
    };
    *module = Box::into_raw(Box::new(new_module)) as *mut wasmer_module_t;
    wasmer_result_t::WASMER_OK
}

fn get_metered_compiler() -> impl Compiler {
    use wasmer_middleware_common::metering;
    use wasmer_runtime_core::codegen::{MiddlewareChain, StreamingCompiler};

    #[cfg(feature = "llvm-backend")]
    use wasmer_llvm_backend::ModuleCodeGenerator as MeteredMCG;

    #[cfg(feature = "singlepass-backend")]
    use wasmer_singlepass_backend::ModuleCodeGenerator as MeteredMCG;

    #[cfg(feature = "cranelift-backend")]
    use wasmer_clif_backend::CraneliftModuleCodeGenerator as MeteredMCG;

    let c: StreamingCompiler<MeteredMCG, _, _, _, _> = StreamingCompiler::new(move || {
        let mut chain = MiddlewareChain::new();
        chain.push(metering::Metering::new());
        chain
    });
    c
}

// returns gas used
#[allow(clippy::cast_ptr_alignment)]
#[no_mangle]
pub unsafe extern "C" fn wasmer_instance_get_points_used(instance: *mut wasmer_instance_t) -> u64 {
    if instance.is_null() {
        return 0;
    }
    use wasmer_middleware_common::metering;
    let instance = &*(instance as *const wasmer_runtime::Instance);
    let points = metering::get_points_used(instance);
    points
}

// returns gas used
#[allow(clippy::cast_ptr_alignment)]
#[no_mangle]
pub unsafe extern "C" fn wasmer_instance_context_get_points_used(
    ctx: *mut wasmer_instance_context_t,
) -> u64 {
    if ctx.is_null() {
        return 0;
    }
    use wasmer_middleware_common::metering;
    let ctx = &*(ctx as *const wasmer_runtime::Ctx);
    let points = metering::get_points_used_ctx(ctx);
    points
}

// sets gas used
#[allow(clippy::cast_ptr_alignment)]
#[no_mangle]
pub unsafe extern "C" fn wasmer_instance_set_points_used(
    instance: *mut wasmer_instance_t,
    new_gas: u64,
) {
    if instance.is_null() {
        return;
    }
    use wasmer_middleware_common::metering;
    let instance = &mut *(instance as *mut wasmer_runtime::Instance);
    metering::set_points_used(instance, new_gas)
}

// sets gas used
#[allow(clippy::cast_ptr_alignment)]
#[no_mangle]
pub unsafe extern "C" fn wasmer_instance_context_set_points_used(
    ctx: *mut wasmer_instance_context_t,
    new_gas: u64,
) {
    if ctx.is_null() {
        return;
    }
    use wasmer_middleware_common::metering;
    let ctx = &mut *(ctx as *mut wasmer_runtime::Ctx);
    metering::set_points_used_ctx(ctx, new_gas)
}

// sets gas limit
#[allow(clippy::cast_ptr_alignment)]
#[no_mangle]
pub unsafe extern "C" fn wasmer_instance_set_execution_limit(
    instance: *mut wasmer_instance_t,
    limit: u64,
) {
    use wasmer_middleware_common::metering;
    let instance = &mut *(instance as *mut wasmer_runtime::Instance);
    metering::set_execution_limit(instance, limit)
}

// get current gas limit
#[allow(clippy::cast_ptr_alignment)]
#[no_mangle]
pub unsafe extern "C" fn wasmer_instance_get_execution_limit(
    instance: *mut wasmer_instance_t,
) -> u64 {
    use wasmer_middleware_common::metering;
    let instance = &mut *(instance as *mut wasmer_runtime::Instance);
    metering::get_execution_limit(instance)
}

// sets gas limit
#[allow(clippy::cast_ptr_alignment)]
#[no_mangle]
pub unsafe extern "C" fn wasmer_instance_context_set_execution_limit(
    ctx: *mut wasmer_instance_context_t,
    limit: u64,
) {
    use wasmer_middleware_common::metering;
    let ctx = &mut *(ctx as *mut wasmer_runtime::Ctx);
    metering::set_execution_limit_ctx(ctx, limit)
}

// get current gas limit
#[allow(clippy::cast_ptr_alignment)]
#[no_mangle]
pub unsafe extern "C" fn wasmer_instance_context_get_execution_limit(
    ctx: *mut wasmer_instance_context_t,
) -> u64 {
    use wasmer_middleware_common::metering;
    let ctx = &*(ctx as *const wasmer_runtime::Ctx);
    metering::get_execution_limit_ctx(ctx)
}
