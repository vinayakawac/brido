use std::time::Instant;

/// Tracks the welcome-text typing animation and its transition to the "brido" title.
pub struct HeaderState {
    full_text: String,
    start: Instant,
    /// Characters per second for the typing effect
    chars_per_sec: f64,
    /// Seconds after typing completes before the text collapses to "brido"
    linger_secs: f64,
}

impl Default for HeaderState {
    fn default() -> Self {
        Self {
            full_text: "hi, welcome to brido".to_string(),
            start: Instant::now(),
            chars_per_sec: 6.0,
            linger_secs: 4.0,
        }
    }
}

impl HeaderState {
    /// Returns the text that should be displayed right now.
    pub fn current_text(&self) -> &str {
        let elapsed = self.start.elapsed().as_secs_f64();
        let typing_duration = self.full_text.len() as f64 / self.chars_per_sec;

        if elapsed < typing_duration {
            // Still typing
            let chars_shown = (elapsed * self.chars_per_sec) as usize;
            let end = self
                .full_text
                .char_indices()
                .nth(chars_shown)
                .map(|(i, _)| i)
                .unwrap_or(self.full_text.len());
            &self.full_text[..end]
        } else if elapsed < typing_duration + self.linger_secs {
            // Full text visible, lingering
            &self.full_text
        } else {
            // Collapsed to title
            "brido"
        }
    }

    /// True once the animation has finished and the header shows only "brido".
    pub fn is_collapsed(&self) -> bool {
        let typing_duration = self.full_text.len() as f64 / self.chars_per_sec;
        self.start.elapsed().as_secs_f64() > typing_duration + self.linger_secs
    }

    pub fn reset(&mut self) {
        self.start = Instant::now();
    }
}
