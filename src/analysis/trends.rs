use std::collections::VecDeque;

const MAX_SAMPLES: usize = 120;
const MIN_LEAK_WINDOW_SECS: u64 = 300;
const MIN_GOROUTINE_INCREASE: u64 = 50;

#[derive(Debug, Default)]
pub struct GoroutineTrend {
    samples: VecDeque<(u64, u64)>,
}

impl GoroutineTrend {
    pub fn record(&mut self, epoch: u64, goroutines: u64) {
        self.samples.push_back((epoch, goroutines));
        while self.samples.len() > MAX_SAMPLES {
            self.samples.pop_front();
        }
    }

    pub fn growth_per_minute(&self) -> Option<f64> {
        let (first_epoch, first_count) = *self.samples.front()?;
        let (last_epoch, last_count) = *self.samples.back()?;
        let elapsed = last_epoch.saturating_sub(first_epoch);
        if elapsed < 60 {
            return None;
        }
        Some((last_count as f64 - first_count as f64) / elapsed as f64 * 60.0)
    }

    pub fn is_suspicious(&self) -> bool {
        let Some(&(first_epoch, first_count)) = self.samples.front() else {
            return false;
        };
        let Some(&(last_epoch, last_count)) = self.samples.back() else {
            return false;
        };
        last_epoch.saturating_sub(first_epoch) >= MIN_LEAK_WINDOW_SECS
            && last_count.saturating_sub(first_count) >= MIN_GOROUTINE_INCREASE
            && self.growth_per_minute().unwrap_or_default() >= 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_sustained_goroutine_growth() {
        let mut trend = GoroutineTrend::default();
        for minute in 0..=5 {
            trend.record(minute * 60, 10 + minute * 12);
        }
        assert!(trend.is_suspicious());
        assert!(trend.growth_per_minute().unwrap() >= 12.0);
    }

    #[test]
    fn ignores_short_spikes() {
        let mut trend = GoroutineTrend::default();
        trend.record(0, 10);
        trend.record(30, 100);
        assert!(!trend.is_suspicious());
    }
}
