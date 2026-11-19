//! Live text-in/text-out tools: the input/output state shared by most tools
//! and the debounce that keeps typing smooth on large inputs.
use eframe::egui;
use super::{clear_button, code_editor, copy_button, error_label, paste_button, properties, sample_button, segmented, toolbar, Highlight, KEY_VALUE};

/// Inputs up to this size are processed on every keystroke; larger ones wait
/// for a short pause in typing.
const LIVE_IMMEDIATE_BYTES: usize = 50_000;
/// Pause in typing, in seconds, before a large input is processed.
const LIVE_DELAY: f64 = 0.35;

/// Delays live processing of large inputs until typing pauses.
#[derive(Default)]
pub struct Debounce {
    /// A change has not been processed yet.
    pending: bool,
    /// `ctx` time (seconds) of the last change.
    since: f64,
}

impl Debounce {
    /// Records that the input changed.
    pub fn touch(&mut self, ctx: &egui::Context) {
        self.pending = true;
        self.since = ctx.input(|i| i.time);
    }

    /// Asks for a run on the next frame, e.g. after the input was set from outside the tool.
    pub fn trigger(&mut self) {
        self.pending = true;
        self.since = f64::NEG_INFINITY;
    }

    /// True once after a change: immediately for small inputs, after a pause
    /// for large ones (a repaint is scheduled so the deadline is not missed).
    pub fn due(&mut self, ctx: &egui::Context, input_len: usize) -> bool {
        if !self.pending { return false; }
        let delay = if input_len > LIVE_IMMEDIATE_BYTES { LIVE_DELAY } else { 0.0 };
        let wait = self.since + delay - ctx.input(|i| i.time);
        if wait > 0.0 {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(wait));
            return false;
        }
        self.pending = false;
        true
    }
}

/// Input, output and error state shared by text-in/text-out tools.
#[derive(Default)]
pub struct TextIo {
    pub input: String,
    pub output: String,
    /// Shown in a red banner above the output; empty when there is none.
    pub error: String,
    /// Syntax highlighting of the input pane (`None` = plain text).
    pub input_syntax: Option<&'static str>,
    /// Syntax highlighting of the output pane (`None` = plain text);
    /// [`KEY_VALUE`] shows the output as a [`properties()`] report.
    pub output_syntax: Option<&'static str>,
    /// Index of the selected action ("mode"): it runs again whenever the input changes.
    pub active: usize,
    /// Pending live update after the input changed.
    pub live: Debounce,
}

impl TextIo {
    /// Stores a transform result: the value goes to the output, an error replaces it.
    pub fn apply(&mut self, result: Result<String, String>) {
        match result { Ok(value) => { self.output = value; self.error.clear(); } Err(error) => { self.output.clear(); self.error = error; } }
    }

    /// Empties the text but keeps the highlighting settings.
    pub fn clear(&mut self) {
        self.input.clear();
        self.output.clear();
        self.error.clear();
    }

    /// Draws the input pane (toolbar + editor). With several actions, a
    /// segmented control picks the mode, which re-runs live as the input
    /// changes. Returns true when the result should be recomputed now
    /// (mode changed, sample loaded, or the input changed).
    pub fn input_pane(&mut self, ui: &mut egui::Ui, actions: &[&str], hint: &str, sample: Option<&str>) -> bool {
        self.input_pane_with(ui, actions, hint, sample, |_| false)
    }

    /// Like [`TextIo::input_pane`], with `extra` controls at the end of the
    /// toolbar (e.g. options of the mode); `extra` returns true to re-run.
    pub fn input_pane_with(&mut self, ui: &mut egui::Ui, actions: &[&str], hint: &str, sample: Option<&str>, extra: impl FnOnce(&mut egui::Ui) -> bool) -> bool {
        let mut run_now = false;
        toolbar(ui, "Input:", |ui| {
            if paste_button(ui, &mut self.input, &mut self.error) { self.live.touch(ui.ctx()); }
            if let Some(sample) = sample {
                if sample_button(ui) {
                    sample.clone_into(&mut self.input);
                    self.active = 0;
                    run_now = true;
                }
            }
            if clear_button(ui) { self.clear(); }
            if actions.len() > 1 {
                ui.separator();
                let options: Vec<(usize, &str)> = actions.iter().copied().enumerate().collect();
                run_now |= segmented(ui, &mut self.active, &options);
            }
            run_now |= extra(ui);
        });
        if code_editor(ui, "input", &mut self.input, hint, Highlight::syntax(self.input_syntax)).changed() {
            self.live.touch(ui.ctx());
        }
        run_now || self.live.due(ui.ctx(), self.input.len())
    }

    /// Draws the output pane: toolbar with Copy and whatever `extra` adds
    /// (it receives the output text), the error if any, then the result.
    pub fn output_pane<R>(&mut self, ui: &mut egui::Ui, extra: impl FnOnce(&mut egui::Ui, &str) -> R) -> R {
        let result = toolbar(ui, "Output:", |ui| {
            copy_button(ui, &self.output);
            extra(ui, &self.output)
        });
        error_label(ui, &self.error);
        if self.output_syntax == Some(KEY_VALUE) {
            properties(ui, "output", &self.output);
        } else {
            code_editor(ui, "output", &mut self.output, "", Highlight::syntax(self.output_syntax));
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs `check` inside a frame whose clock reads `time` seconds.
    fn at(ctx: &egui::Context, time: f64, check: impl FnOnce(&egui::Context)) {
        let mut check = Some(check);
        let _ = ctx.run(egui::RawInput { time: Some(time), ..Default::default() }, |ctx| {
            if let Some(check) = check.take() { check(ctx); }
        });
    }

    #[test]
    fn small_inputs_run_at_once() {
        let ctx = egui::Context::default();
        let mut live = Debounce::default();
        at(&ctx, 1.0, |ctx| {
            assert!(!live.due(ctx, 10), "nothing changed yet");
            live.touch(ctx);
            assert!(live.due(ctx, 10));
            // Due once per change.
            assert!(!live.due(ctx, 10));
        });
    }

    #[test]
    fn large_inputs_wait_for_a_pause() {
        let ctx = egui::Context::default();
        let mut live = Debounce::default();
        let large = LIVE_IMMEDIATE_BYTES + 1;
        at(&ctx, 1.0, |ctx| { live.touch(ctx); assert!(!live.due(ctx, large)); });
        at(&ctx, 1.0 + LIVE_DELAY / 2.0, |ctx| assert!(!live.due(ctx, large)));
        at(&ctx, 1.0 + LIVE_DELAY, |ctx| assert!(live.due(ctx, large)));
        // A change from outside the tool runs on the next frame, whatever the size.
        live.trigger();
        at(&ctx, 2.0, |ctx| assert!(live.due(ctx, large)));
    }

    #[test]
    fn results_and_errors_replace_each_other() {
        let mut io = TextIo { input: "in".into(), input_syntax: Some("json"), output_syntax: Some("json"), ..TextIo::default() };
        io.apply(Ok("out".into()));
        assert_eq!((io.output.as_str(), io.error.as_str()), ("out", ""));
        io.apply(Err("bad".into()));
        assert_eq!((io.output.as_str(), io.error.as_str()), ("", "bad"));
        io.apply(Ok("again".into()));
        assert_eq!((io.output.as_str(), io.error.as_str()), ("again", ""));
        // Clearing empties the text but keeps the highlighting.
        io.error = "x".into();
        io.clear();
        assert!(io.input.is_empty() && io.output.is_empty() && io.error.is_empty());
        assert_eq!((io.input_syntax, io.output_syntax), (Some("json"), Some("json")));
    }
}
