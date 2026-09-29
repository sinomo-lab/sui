//! Display-paced animation frames: predict when the frame being prepared
//! will reach the screen, on a steady cadence of display refreshes.

/// Shortest and longest refresh intervals the cadence accepts, in seconds.
const MIN_INTERVAL: f64 = 1.0 / 500.0;
const MAX_INTERVAL: f64 = 1.0 / 20.0;
/// How strongly each frame pulls the cadence toward the measured time, so a
/// display whose real rate differs slightly from the reported one (59.94 Hz
/// reported as 60 Hz) stays in phase without jitter.
const PHASE_CORRECTION: f64 = 0.1;

/// Predicts presentation times for display-paced animation frames.
///
/// Frame callbacks arrive with scheduling jitter, but frames reach the
/// screen on whole display refreshes. Stamping animation frames with the
/// jittery callback time makes motion stutter, so the cadence snaps each
/// frame to a whole number of refresh intervals after the previous one,
/// slowly pulled toward the measured time to stay in phase.
#[derive(Debug, Clone, Default)]
pub(crate) struct FrameCadence {
    last_frame: Option<f64>,
    last_callback: Option<f64>,
    estimated_interval: Option<f64>,
}

impl FrameCadence {
    /// The expected presentation time of a frame being prepared at `now`.
    /// `refresh_interval` is the display's refresh period when the platform
    /// reports one; otherwise the cadence estimates it from callback timing.
    pub(crate) fn next(&mut self, now: f64, refresh_interval: Option<f64>) -> f64 {
        let measured = self.observe_callback(now);
        let interval = refresh_interval
            .or(measured)
            .unwrap_or(1.0 / 60.0)
            .clamp(MIN_INTERVAL, MAX_INTERVAL);
        // The frame being prepared now is shown on the next refresh.
        let target = now + interval;
        let frame = match self.last_frame {
            Some(last) => {
                let steps = ((target - last) / interval).round().max(1.0);
                let locked = last + steps * interval;
                let corrected = locked + (target - locked) * PHASE_CORRECTION;
                // Never repeat or go back in time.
                corrected.max(last + interval * 0.5)
            }
            None => target,
        };
        self.last_frame = Some(frame);
        frame
    }

    /// Track the interval between consecutive callbacks, ignoring gaps where
    /// nothing was animating.
    fn observe_callback(&mut self, now: f64) -> Option<f64> {
        if let Some(previous) = self.last_callback {
            let gap = now - previous;
            let plausible = gap > MIN_INTERVAL
                && gap < MAX_INTERVAL
                && self
                    .estimated_interval
                    .is_none_or(|estimate| gap < estimate * 3.0);
            if plausible {
                self.estimated_interval = Some(match self.estimated_interval {
                    Some(estimate) => estimate + (gap - estimate) * 0.1,
                    None => gap,
                });
            }
        }
        self.last_callback = Some(now);
        self.estimated_interval
    }
}

#[cfg(test)]
mod tests {
    use super::FrameCadence;

    const INTERVAL: f64 = 1.0 / 144.0;

    #[test]
    fn frames_land_on_whole_refreshes_despite_callback_jitter() {
        let mut cadence = FrameCadence::default();
        let jitter = [0.0, 0.0012, -0.0009, 0.0015, -0.0011, 0.0004];
        let mut frames = Vec::new();
        for (index, offset) in jitter.iter().enumerate() {
            frames.push(cadence.next(index as f64 * INTERVAL + offset, Some(INTERVAL)));
        }

        for pair in frames.windows(2) {
            let step = pair[1] - pair[0];
            assert!(
                (step - INTERVAL).abs() < INTERVAL * 0.15,
                "steps stay near one refresh: {step}"
            );
        }
    }

    #[test]
    fn a_late_callback_skips_whole_refreshes() {
        let mut cadence = FrameCadence::default();
        let first = cadence.next(0.0, Some(INTERVAL));
        let second = cadence.next(3.0 * INTERVAL, Some(INTERVAL));

        let steps = (second - first) / INTERVAL;
        assert!((steps - 3.0).abs() < 0.15, "skipped {steps} refreshes");
    }

    #[test]
    fn frame_times_always_move_forward() {
        let mut cadence = FrameCadence::default();
        let first = cadence.next(1.0, Some(INTERVAL));
        let second = cadence.next(1.0, Some(INTERVAL));

        assert!(second > first);
        assert!(
            (first - (1.0 + INTERVAL)).abs() < 1e-12,
            "shown on the next refresh"
        );
    }

    #[test]
    fn the_interval_is_estimated_when_the_display_does_not_report_one() {
        let mut cadence = FrameCadence::default();
        let interval = 1.0 / 90.0;
        let mut last = 0.0;
        for frame in 0..40 {
            last = cadence.next(f64::from(frame) * interval, None);
        }
        let next = cadence.next(40.0 * interval, None);

        assert!(((next - last) - interval).abs() < interval * 0.1);
    }
}
