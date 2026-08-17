macro_rules! state_telemetry_enabled_metric_methods {
    ($( $(#[$attr:meta])* [$name:ident($($arg:ident: $arg_ty:ty),* $(,)?) => $($op:tt)*] )+) => {
        $(
            $(#[$attr])*
            pub fn $name(&self $(, $arg: $arg_ty)*) {
                if self.is_enabled() {
                    self.metrics $($op)*
                }
            }
        )+
    };
}

macro_rules! state_telemetry_enabled_metric_methods_early_return {
    ($( $(#[$attr:meta])* [$name:ident($($arg:ident: $arg_ty:ty),* $(,)?) => $($op:tt)*] )+) => {
        $(
            $(#[$attr])*
            pub fn $name(&self $(, $arg: $arg_ty)*) {
                if !self.is_enabled() {
                    return;
                }
                self.metrics $($op)*
            }
        )+
    };
}
