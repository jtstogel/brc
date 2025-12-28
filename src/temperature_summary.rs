pub struct TemperatureSummary {
    min: i32,
    max: i32,
    total: i64,
    count: i32,
}

impl TemperatureSummary {
    pub fn min(&self) -> i32 {
        self.min
    }

    pub fn max(&self) -> i32 {
        self.max
    }

    pub fn avg(&self) -> f32 {
        ((self.total as f64 / self.count as f64) / 10f64) as f32
    }

    #[cfg_attr(feature = "profiled", inline(never))]
    pub fn add_reading(&mut self, temp: i32) {
        self.min = self.min.min(temp);
        self.max = self.max.max(temp);
        self.total += temp as i64;
        self.count += 1;
    }
}

impl Default for TemperatureSummary {
    fn default() -> Self {
        Self {
            min: i32::MAX,
            max: i32::MIN,
            total: 0,
            count: 0,
        }
    }
}

impl TemperatureSummary {
    pub fn of(temp: i32) -> Self {
        Self {
            min: temp,
            max: temp,
            total: temp as i64,
            count: 1,
        }
    }
}
