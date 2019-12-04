use wasmer_runtime_core::{
    codegen::{Event, EventSink, FunctionMiddleware},
    module::ModuleInfo,
};

pub struct EventTrace {}

impl EventTrace {
    pub fn new() -> EventTrace {
        EventTrace {}
    }
}

impl FunctionMiddleware for EventTrace {
    type Error = String;
    fn feed_event<'a, 'b: 'a>(
        &mut self,
        op: Event<'a, 'b>,
        _module_info: &ModuleInfo,
        sink: &mut EventSink<'a, 'b>,
    ) -> Result<(), Self::Error> {
        println!("{:?}", op);
        sink.push(op);
        Ok(())
    }
}
