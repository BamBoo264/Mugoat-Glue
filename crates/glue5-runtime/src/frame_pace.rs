//! The engine's frame limiter (`-frameLimit`, at the end of
//! `CSystem::EndRender` on the render thread) waits for 95% of the frame's
//! time since the last frame: in sleeps of whole milliseconds, less its
//! worst oversleep so far (1.5 ms at least, up to 16.5, forgotten 0.1% a
//! frame), then spinning on the clock for the rest. Apple's and Android's
//! sleeps run up to a fifth long, so that worst sits at 2-6 ms and the
//! render thread spun 0.5-3.5 ms a frame (a tenth of a core at 60 or 30
//! fps, which a phone's GPU and loading screens want).
//!
//! The engine's limiter now finds every frame late and lets it go; the
//! host then waits for the same deadline, in sleeps of half what is left
//! (none too long however late each comes back), and stores the frame's
//! end as the engine would have.

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use glue5_game::{Env, futex, orig};

use crate::memory::Mem;

/// When the limiter last let a frame go (`sysTimer` ticks, u64), and the
/// seconds a tick lasts (f32). Addresses in this build.
const LAST_END: u64 = 25038496;
const TICK_SECONDS: u64 = 9779848;
/// The display's refresh rate (numerator, denominator) and the refresh
/// intervals a frame lasts (the VSync setting `-frameLimit` seeds; 0: off).
const REFRESH: u64 = 12622824;
const INTERVALS: u64 = 19447080;
/// What the limiter lets a frame take when it is off: 1/256 s.
const UNLIMITED: f32 = 1.0 / 256.0;

/// 1 while the app is in the background (phones): the render thread stops
/// at the end of its frame, and with it the game. Android lets the app run
/// there; iOS runs it for a while before suspending it, failing its GPU
/// work meanwhile.
static BACKGROUND: AtomicU32 = AtomicU32::new(0);
/// Host thermal FPS ceiling (`0` = disabled, use user/engine cap only).
static THERMAL_MAX_FPS: AtomicU32 = AtomicU32::new(0);

/// Sets an adaptive thermal frame-rate ceiling (`0` disables throttling).
pub fn set_thermal_cap(fps: u32) {
    THERMAL_MAX_FPS.store(fps, Ordering::Release);
}

/// Active thermal frame-rate ceiling (`0` = none).
pub fn thermal_cap() -> u32 {
    THERMAL_MAX_FPS.load(Ordering::Acquire)
}

/// Hysteresis thermal throttle policy for mobile play (`0 nominal .. 3 critical`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ThermalPolicy {
    active_tier: u8,
    serious_since_ms: Option<u64>,
    cool_since_ms: Option<u64>,
}

impl ThermalPolicy {
    /// Evaluates thermal state at `now_ms`, returning `(effective_cap_fps, should_trim_gpu)`.
    /// Does not modify user-saved preferences (`user_max_fps`).
    pub fn step(&mut self, thermal_state: isize, now_ms: u64, user_max_fps: u32) -> (u32, bool) {
        let mut trim = false;
        if thermal_state >= 3 {
            self.cool_since_ms = None;
            if self.active_tier < 2 {
                trim = true;
            }
            self.active_tier = 2;
        } else if thermal_state == 2 {
            self.cool_since_ms = None;
            let start = *self.serious_since_ms.get_or_insert(now_ms);
            if self.active_tier < 1 && now_ms.saturating_sub(start) >= 3_000 {
                self.active_tier = 1;
            }
        } else {
            self.serious_since_ms = None;
            if self.active_tier > 0 {
                let start = *self.cool_since_ms.get_or_insert(now_ms);
                if now_ms.saturating_sub(start) >= 15_000 {
                    self.active_tier = 0;
                    self.cool_since_ms = None;
                }
            }
        }
        let tier_cap = match self.active_tier {
            2 => 30,
            1 => 40,
            _ => 0,
        };
        let effective = match (tier_cap, user_max_fps) {
            (0, u) => u,
            (t, 0) => t,
            (t, u) => t.min(u),
        };
        (effective, trim)
    }
}

/// The app went to the background, or came back.
pub fn set_background(on: bool) {
    BACKGROUND.store(on as u32, Ordering::Release);
    if !on {
        futex::notify(&BACKGROUND, u32::MAX);
    }
}

/// Hook for `CSystem::EndRender()`.
pub(crate) unsafe extern "C" fn end_render(env: *mut Env) {
    while BACKGROUND.load(Ordering::Acquire) == 1 {
        futex::wait32(&BACKGROUND, 1, None);
    }
    let mem = unsafe { Mem::of(env) };
    crate::memory::report_heaps(mem);
    let last = mem.atomic_u64(LAST_END).swap(0, Ordering::AcqRel);
    unsafe { orig::end_render(env) };
    // The engine's computation, as it would have made it (f32 included).
    let now = mem.atomic_u64(LAST_END).load(Ordering::Acquire);
    if last == 0 || now < last {
        return;
    }
    let tick = f32::from_bits(mem.u32(TICK_SECONDS));
    let hz = (mem.u32(REFRESH) / mem.u32(REFRESH + 4).max(1)) as i32;
    let intervals = mem.i32(INTERVALS);
    let mut target = if hz > 0 && intervals > 0 { intervals as f32 / hz as f32 } else { UNLIMITED };
    let therm_cap = THERMAL_MAX_FPS.load(Ordering::Acquire);
    if therm_cap > 0 {
        target = target.max(1.0 / therm_cap as f32);
    }
    let elapsed = (now - last) as f32 * tick;
    let wait = 0.95 * target - elapsed;
    if wait.is_nan() || wait <= 0.0 || tick.is_nan() || tick <= 0.0 {
        return;
    }
    let started = Instant::now();
    sleep_until(started + Duration::from_secs_f32(wait));
    let waited = started.elapsed().as_secs_f32() / tick;
    mem.atomic_u64(LAST_END).store(now + waited as u64, Ordering::Release);
}

/// Sleeps until `deadline`: half of what is left at a time, as the system's
/// sleeps can run a fifth long.
fn sleep_until(deadline: Instant) {
    while let Some(left) = deadline.checked_duration_since(Instant::now()).filter(|l| *l > Duration::from_micros(50)) {
        std::thread::sleep(left / 2);
    }
}

/// This module's hooks.
pub(crate) fn hooks(hooks: &mut glue5_game::Hooks) {
    hooks.end_render = Some(end_render);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thermal_policy_steps_down_with_hysteresis_and_recovers() {
        let mut p = ThermalPolicy::default();
        assert_eq!(p.step(0, 1_000, 60), (60, false));
        // Brief Serious spike (< 3s) does not immediately jerk framerate.
        assert_eq!(p.step(2, 2_000, 60), (60, false));
        // Sustained Serious >= 3s steps cap smoothly to 40 FPS.
        assert_eq!(p.step(2, 5_500, 60), (40, false));
        // Momentary drop back to Fair keeps 40 FPS hysteresis active until 15s sustained cool.
        assert_eq!(p.step(1, 10_000, 60), (40, false));
        assert_eq!(p.step(0, 26_000, 60), (60, false));
        // Critical immediately caps to 30 FPS and requests GPU cache trim.
        assert_eq!(p.step(3, 30_000, 60), (30, true));
        // User cap lower than thermal cap (e.g. 24) stays respected.
        assert_eq!(p.step(3, 31_000, 24), (24, false));
    }
}
