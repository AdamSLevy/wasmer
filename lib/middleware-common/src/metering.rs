use wasmer_runtime_core::{
    codegen::{Event, EventSink, FunctionMiddleware, InternalEvent},
    module::ModuleInfo,
    vm::{Ctx, InternalField},
    wasmparser::{Operator, Type as WpType, TypeOrFuncType as WpTypeOrFuncType},
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
pub struct Metering {
    cost_operator_idxs: Vec<usize>,
    current_block_cost: u64,
}

impl Metering {
    pub fn new() -> Metering {
        Metering {
            cost_operator_idxs: Vec::new(),
            current_block_cost: 0,
        }
    }

    /// inject_metering injects a series of opcodes that adds the cost of the next code block to
    /// INTERNAL_FIELD_USED and then checks if it has exceeded INTERNAL_FIELD_LIMIT. Since the cost
    /// of the next code block is not known at the point of injection, Operator::Unreachable is
    /// used in place of Operator::I64Const{COST}, and the position of this Operator in the sink is
    /// saved, so that it can later be replaced with the correct cost, once it is know later on in
    /// parsing Events.
    fn inject_metering<'a, 'b: 'a>(&mut self, sink: &mut EventSink<'a, 'b>) {
        // PUSH USED
        sink.push(Event::Internal(InternalEvent::GetInternal(
            INTERNAL_FIELD_USED.index() as _,
        )));

        // placeholder for PUSH COST
        self.cost_operator_idxs.push(sink.buffer.len());
        sink.push(Event::WasmOwned(Operator::I64Const { value: 0 }));

        // USED + COST
        sink.push(Event::WasmOwned(Operator::I64Add));

        // SAVE USED
        sink.push(Event::Internal(InternalEvent::SetInternal(
            INTERNAL_FIELD_USED.index() as _,
        )));

        // PUSH USED
        sink.push(Event::Internal(InternalEvent::GetInternal(
            INTERNAL_FIELD_USED.index() as _,
        )));

        // PUSH LIMIT
        sink.push(Event::Internal(InternalEvent::GetInternal(
            INTERNAL_FIELD_LIMIT.index() as _,
        )));

        // IF USED > LIMIT
        sink.push(Event::WasmOwned(Operator::I64GtU));
        sink.push(Event::WasmOwned(Operator::If {
            ty: WpTypeOrFuncType::Type(WpType::EmptyBlockType),
        }));

        //          TRAP! EXECUTION LIMIT EXCEEDED
        sink.push(Event::Internal(InternalEvent::Breakpoint(Box::new(|_| {
            Err(Box::new(ExecutionLimitExceededError))
        }))));

        // ENDIF
        sink.push(Event::WasmOwned(Operator::End));
    }

    fn remove_trailing_injection<'a, 'b: 'a>(&mut self, sink: &mut EventSink<'a, 'b>) {
        if let Event::WasmOwned(Operator::End) = sink.buffer[sink.buffer.len() - 11] {
            // Remove the last 10 Operators.
            sink.buffer.truncate(sink.buffer.len() - 10);
        }
    }

    fn set_costs<'a, 'b: 'a>(&mut self, sink: &mut EventSink<'a, 'b>) {
        for idx in &self.cost_operator_idxs {
            match sink.buffer[*idx] {
                Event::WasmOwned(Operator::I64Const { value }) => {
                    sink.buffer[*idx] = Event::WasmOwned(Operator::I64Const {
                        value: value + (self.current_block_cost as i64),
                    });
                }
                _ => panic!(),
            }
        }
        self.current_block_cost = 0;
    }

    fn begin<'a, 'b: 'a>(&mut self, sink: &mut EventSink<'a, 'b>) {
        self.set_costs(sink);
        self.inject_metering(sink);
    }
    fn end<'a, 'b: 'a>(&mut self, sink: &mut EventSink<'a, 'b>) {
        self.set_costs(sink);
        self.cost_operator_idxs.clear();
    }

    /// increment_cost adds 1 to the current_block_cost.
    ///
    /// Later this may be replaced with a cost map for assigning custom unique cost values to
    /// specific Operators.
    fn increment_cost<'a, 'b: 'a>(&mut self, _op: &Event<'a, 'b>) {
        self.current_block_cost += 1;
    }
}

#[derive(Copy, Clone, Debug)]
pub struct ExecutionLimitExceededError;

impl FunctionMiddleware for Metering {
    type Error = String;
    fn feed_event<'a, 'b: 'a>(
        &mut self,
        ev: Event<'a, 'b>,
        _module_info: &ModuleInfo,
        sink: &mut EventSink<'a, 'b>,
    ) -> Result<(), Self::Error> {
        self.increment_cost(&ev);

        let op_idx;
        match ev {
            Event::Internal(ref iev) => match iev {
                InternalEvent::FunctionBegin(_) => {
                    sink.push(ev);
                    self.begin(sink);
                    return Ok(());
                }
                InternalEvent::FunctionEnd => {
                    self.end(sink);
                    self.remove_trailing_injection(sink);
                    sink.push(ev);
                    return Ok(());
                }
                _ => {
                    sink.push(ev);
                    return Ok(());
                }
            },
            Event::Wasm(&ref op) | Event::WasmOwned(ref op) => {
                match *op {
                    Operator::End
                    | Operator::If { .. }
                    | Operator::Else
                    | Operator::Br { .. }
                    | Operator::BrIf { .. }
                    | Operator::BrTable { .. }
                    | Operator::Return => {
                        self.end(sink);
                    }
                    _ => {}
                }

                op_idx = sink.buffer.len();
                sink.push(Event::WasmOwned(Operator::Unreachable));
                match *op {
                    Operator::Loop { .. }
                    | Operator::End
                    | Operator::If { .. }
                    | Operator::Else
                    | Operator::BrIf { .. } => {
                        self.begin(sink);
                    }
                    _ => {}
                }
            }
        }

        sink.buffer[op_idx] = ev;

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
