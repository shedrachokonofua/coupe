use std::time::Duration;

use opentelemetry::{ metrics::{ Counter, Histogram, Meter, UpDownCounter }, KeyValue };

pub struct CoupeFunctionMetrics {
    invocations: Counter<u64>,
    invoke_duration: Histogram<u64>,
    active_invocations: UpDownCounter<i64>,
    init_duration: Histogram<f64>,
    cold_starts: Counter<u64>,
    errors: Counter<u64>,
}

impl CoupeFunctionMetrics {
    pub fn new(meter: Meter) -> Self {
        Self {
            invocations: meter
                .u64_counter(opentelemetry_semantic_conventions::metric::FAAS_INVOCATIONS)
                .with_description("Number of successful invocations")
                .build(),
            invoke_duration: meter
                .u64_histogram(opentelemetry_semantic_conventions::metric::FAAS_INVOKE_DURATION)
                .with_description("Duration of successful invocations")
                .with_unit("ns")
                .build(),
            active_invocations: meter
                .i64_up_down_counter("faas.active_invocations")
                .with_description("Number of active invocations")
                .build(),
            init_duration: meter
                .f64_histogram(opentelemetry_semantic_conventions::metric::FAAS_INIT_DURATION)
                .with_description("Duration of function initialization")
                .with_unit("ns")
                .build(),
            cold_starts: meter
                .u64_counter(opentelemetry_semantic_conventions::metric::FAAS_COLDSTARTS)
                .with_description("Number of cold starts")
                .build(),
            errors: meter
                .u64_counter(opentelemetry_semantic_conventions::metric::FAAS_ERRORS)
                .with_description("Number of errors")
                .build(),
        }
    }

    pub fn record_begin_invoke(&self, tags: &[KeyValue]) {
        self.invocations.add(1, tags);
        self.active_invocations.add(1, tags);
    }

    pub fn record_end_invoke(&self, duration: Duration, is_error: bool, tags: &[KeyValue]) {
        self.invoke_duration.record(duration.as_nanos() as u64, tags);
        self.active_invocations.add(-1, tags);

        if is_error {
            self.errors.add(1, tags);
        }
    }

    pub fn record_init(&self, duration: Duration, is_cold_start: bool, tags: &[KeyValue]) {
        self.init_duration.record(duration.as_nanos() as f64, tags);
        if is_cold_start {
            self.cold_starts.add(1, tags);
        }
    }
}
