use std::cell::Cell;

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

    pub fn avg(&self) -> f32 {
        ((self.total.get() as f64 / self.count.get() as f64) / 10f64) as f32
    }

    #[cfg_attr(feature = "profiled", inline(never))]
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
