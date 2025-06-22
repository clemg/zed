use crate::EditorSettings;
use gpui::{Context, Point, Pixels};
use settings::Settings;
use settings::SettingsStore;
use std::time::{Duration, Instant};

const ANIMATION_DURATION_MS: u64 = 120;

#[derive(Clone, Debug)]
pub struct AnimatedCursorPosition {
    pub start_point: Point<Pixels>,
    pub end_point: Point<Pixels>,
    pub start_time: Instant,
    pub duration: Duration,
}

impl AnimatedCursorPosition {
    pub fn new(start: Point<Pixels>, end: Point<Pixels>) -> Self {
        Self {
            start_point: start,
            end_point: end,
            start_time: Instant::now(),
            duration: Duration::from_millis(ANIMATION_DURATION_MS),
        }
    }

    pub fn current_position(&self) -> Point<Pixels> {
        let elapsed = self.start_time.elapsed();
        
        if elapsed >= self.duration {
            return self.end_point;
        }
        
        let progress = elapsed.as_millis() as f32 / self.duration.as_millis() as f32;
        
        // Use smooth lerp (easing)
        let smooth_progress = Self::ease_out_cubic(progress);
        
        Point::new(
            self.start_point.x + (self.end_point.x - self.start_point.x) * smooth_progress,
            self.start_point.y + (self.end_point.y - self.start_point.y) * smooth_progress,
        )
    }

    pub fn is_complete(&self) -> bool {
        self.start_time.elapsed() >= self.duration
    }

    // Cubic ease-out function for smooth animation
    fn ease_out_cubic(t: f32) -> f32 {
        1.0 - (1.0 - t).powi(3)
    }
}

pub struct CursorAnimationManager {
    animation_epoch: usize,
    enabled: bool,
    active_animation: Option<AnimatedCursorPosition>,
}

impl CursorAnimationManager {
    pub fn new(cx: &mut Context<Self>) -> Self {
        // Observe settings changes to enable/disable animation
        cx.observe_global::<SettingsStore>(move |this, cx| {
            let enabled = EditorSettings::get_global(cx).cursor_smooth_animation;
            if this.enabled != enabled {
                this.enabled = enabled;
                if !enabled {
                    this.active_animation = None;
                }
            }
        })
        .detach();

        Self {
            animation_epoch: 0,
            enabled: false,
            active_animation: None,
        }
    }

    fn next_animation_epoch(&mut self) -> usize {
        self.animation_epoch += 1;
        self.animation_epoch
    }

    pub fn start_animation(
        &mut self,
        start_position: Point<Pixels>,
        end_position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if !self.enabled {
            return;
        }

        // Skip animation if positions are the same
        if start_position == end_position {
            return;
        }

        self.active_animation = Some(AnimatedCursorPosition::new(start_position, end_position));

        let epoch = self.next_animation_epoch();
        self.schedule_animation_frame(epoch, cx);
    }

    fn schedule_animation_frame(&mut self, epoch: usize, cx: &mut Context<Self>) {
        if !self.enabled {
            return;
        }

        // Use a faster frame interval for smoother animation (8ms ≈ 120 FPS)
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(8)).await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |this, cx| this.update_animation(epoch, cx))
                    .ok();
            }
        })
        .detach();
    }

    fn update_animation(&mut self, epoch: usize, cx: &mut Context<Self>) {
        if epoch != self.animation_epoch || !self.enabled {
            return;
        }

        if let Some(ref animation) = self.active_animation {
            if animation.is_complete() {
                self.active_animation = None;
            } else {
                // Continue animation - trigger redraw
                cx.notify();
                self.schedule_animation_frame(epoch, cx);
            }
        }
    }

    pub fn current_cursor_position(&self, static_position: Point<Pixels>) -> Point<Pixels> {
        if !self.enabled {
            return static_position;
        }

        if let Some(ref animation) = self.active_animation {
            if !animation.is_complete() {
                return animation.current_position();
            }
        }

        static_position
    }

    pub fn is_animating(&self) -> bool {
        self.enabled && 
        self.active_animation.as_ref().map_or(false, |a| !a.is_complete())
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.active_animation = None;
        }
    }
}