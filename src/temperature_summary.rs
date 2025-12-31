use std::cell::Cell;

#[repr(align(32))]
pub struct TemperatureSummary {
    min: Cell<i32>,
    max: Cell<i32>,
    total: Cell<i64>,
    count: Cell<i32>,
}

impl TemperatureSummary {
    pub fn min(&self) -> i32 {
        self.min.get()
    }

    pub fn max(&self) -> i32 {
        self.max.get()
    }

    pub fn avg(&self) -> f64 {
        let rounded_total = self.total.get() + (self.count.get() / 2) as i64;
        rounded_total.div_euclid(self.count.get() as i64) as f64 / 10.0
    }

    #[cfg_attr(feature = "profiled", inline(never))]
    #[cfg_attr(not(feature = "profiled"), inline(always))]
    pub fn add_reading(&self, temp: i32) {
        self.min.set(self.min.get().min(temp));
        self.max.set(self.max.get().max(temp));
        self.total.set(self.total.get() + temp as i64);
        self.count.set(self.count.get() + 1);
    }
}

impl Default for TemperatureSummary {
    fn default() -> Self {
        Self {
            min: Cell::new(i32::MAX),
            max: Cell::new(i32::MIN),
            total: Cell::new(0),
            count: Cell::new(0),
        }
    }
}

impl TemperatureSummary {
    pub fn of(temp: i32) -> Self {
        Self {
            min: Cell::new(temp),
            max: Cell::new(temp),
            total: Cell::new(temp as i64),
            count: Cell::new(1),
        }
    }
}
