//use std::rc::Rc;
use wasmer_runtime_core::{
    codegen::{Event, EventSink, FunctionMiddleware, InternalEvent},
    module::ModuleInfo,
    vm::{Ctx, InternalField},
    //wasmparser::{Operator, Type as WpType, TypeOrFuncType as WpTypeOrFuncType},
    Instance,
};

static INTERNAL_FIELD_USED: InternalField = InternalField::allocate();
static INTERNAL_FIELD_LIMIT: InternalField = InternalField::allocate();

/// Metering is a compiler middleware that calculates the cost of WebAssembly instructions at compile
/// time and will count the cost of executed instructions at runtime. Within the Metering functionality,
/// this instruction cost is called `points`.
///
/// The Metering struct takes a `limit` parameter which is the maximum number of points which can be
/// used by an instance during a function call. If this limit is exceeded, the function call will
/// trap. Each instance has a `points_used` field which can be used to track points used during
/// a function call and should be set back to zero after a function call.
///
/// Each compiler backend with Metering enabled should produce the same cost used at runtime for
/// the same function calls so we can say that the metering is deterministic.
///
pub struct Metering<'v> {
    operators: Vec<Event<'v>>,
}

impl<'v> Metering<'v> {
    pub fn new() -> Metering<'v> {
        Metering {
            operators: Vec::new(),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct ExecutionLimitExceededError;

impl<'v> FunctionMiddleware for Metering<'v> {
    type Error = String;
    fn feed_event<'a>(
        &mut self,
        ev: Event<'a>,
        _module_info: &ModuleInfo,
        sink: &mut EventSink<'a>,
    ) -> Result<(), Self::Error> {
        match ev {
            Event::Internal(ref iev) => match iev {
                InternalEvent::FunctionBegin(..) => {
                    sink.push(ev);
                    return Ok(());
                }
                InternalEvent::FunctionEnd => {
                    for e in self.operators {
                        sink.push(ev);
                    }
                    sink.push(ev);
                    return Ok(());
                }
                _ => {}
            },
            Event::Wasm(ref op) => self.operators.push(Event::Wasm(op.clone())),
            _ => {}
        }

        Ok(())
    }
}

/// Returns the number of points used by an Instance.
pub fn get_points_used(instance: &Instance) -> u64 {
    instance.get_internal(&INTERNAL_FIELD_USED)
}

/// Sets the number of points used by an Instance.
pub fn set_points_used(instance: &mut Instance, value: u64) {
    instance.set_internal(&INTERNAL_FIELD_USED, value);
}

/// Returns the number of points used in a Ctx.
pub fn get_points_used_ctx(ctx: &Ctx) -> u64 {
    ctx.get_internal(&INTERNAL_FIELD_USED)
}

/// Sets the number of points used in a Ctx.
pub fn set_points_used_ctx(ctx: &mut Ctx, value: u64) {
    ctx.set_internal(&INTERNAL_FIELD_USED, value);
}

pub fn set_execution_limit(instance: &mut Instance, limit: u64) {
    instance.set_internal(&INTERNAL_FIELD_LIMIT, limit);
}

pub fn set_execution_limit_ctx(ctx: &mut Ctx, limit: u64) {
    ctx.set_internal(&INTERNAL_FIELD_LIMIT, limit);
}

pub fn get_execution_limit(instance: &Instance) -> u64 {
    instance.get_internal(&INTERNAL_FIELD_LIMIT)
}

pub fn get_execution_limit_ctx(ctx: &Ctx) -> u64 {
    ctx.get_internal(&INTERNAL_FIELD_LIMIT)
}
