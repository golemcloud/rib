// Copyright 2024-2025 Golem Cloud
//
// Licensed under the Golem Source License v1.0 (the "License");
// you may not use this file except in compliance with the License.
//
// Wall-clock sections for coarse profiling. Enable with:
//   RIB_PROFILE=1   or   RIB_PROFILE=true
//
// Each timed scope records one row; at end of `RibCompiler::compile` (or standalone
// `infer_types`) a sorted table is printed to stderr with % of wall time. Labels that
// duplicate nested work (aggregate timers) are omitted from the table.

use std::cell::{Cell, RefCell};
use std::cmp::Reverse;
use std::io::{stderr, IsTerminal};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;
use std::time::{Duration, Instant};

/// ANSI styling for the summary table. Disabled when stderr is not a TTY, `NO_COLOR` is set, or
/// `RIB_PROFILE_COLOR=0`.
struct Styles {
    on: bool,
}

impl Styles {
    fn detect() -> Self {
        let tty = stderr().is_terminal();
        let no_color = std::env::var_os("NO_COLOR").is_some();
        let force_off = std::env::var_os("RIB_PROFILE_COLOR").is_some_and(|v| {
            let s = v.to_string_lossy();
            s == "0" || s.eq_ignore_ascii_case("off") || s.eq_ignore_ascii_case("never")
        });
        Self {
            on: tty && !no_color && !force_off,
        }
    }

    #[inline]
    fn r(&self) -> &'static str {
        if self.on {
            "\x1b[0m"
        } else {
            ""
        }
    }

    /// Dim bold gray `[rib-profile]` tag (caller prints the brackets and text).
    fn tag(&self) -> &'static str {
        if self.on {
            "\x1b[1;90m"
        } else {
            ""
        }
    }

    fn title(&self) -> &'static str {
        if self.on {
            "\x1b[1;36m"
        } else {
            ""
        }
    }

    fn header(&self) -> &'static str {
        if self.on {
            "\x1b[1m"
        } else {
            ""
        }
    }

    fn sep(&self) -> &'static str {
        if self.on {
            "\x1b[2;90m"
        } else {
            ""
        }
    }

    fn phase_infer(&self) -> &'static str {
        if self.on {
            "\x1b[38;5;121m"
        } else {
            ""
        }
    }

    fn phase_compile(&self) -> &'static str {
        if self.on {
            "\x1b[38;5;117m"
        } else {
            ""
        }
    }

    fn time_col(&self) -> &'static str {
        if self.on {
            "\x1b[1;37m"
        } else {
            ""
        }
    }

    fn pct(&self, pct: f64) -> &'static str {
        if !self.on {
            return "";
        }
        if pct >= 20.0 {
            "\x1b[1;38;5;208m"
        } else if pct >= 8.0 {
            "\x1b[38;5;214m"
        } else if pct >= 1.0 {
            "\x1b[38;5;109m"
        } else {
            "\x1b[2;90m"
        }
    }

    fn footer_label(&self) -> &'static str {
        if self.on {
            "\x1b[2;37m"
        } else {
            ""
        }
    }

    fn footer_note(&self) -> &'static str {
        if self.on {
            "\x1b[38;5;245m"
        } else {
            ""
        }
    }
}

static PROFILE_INIT: Once = Once::new();
static PROFILE_ENABLED: AtomicBool = AtomicBool::new(false);

thread_local! {
    static PROFILE_ENTRIES: RefCell<Vec<(&'static str, Duration)>> = const { RefCell::new(Vec::new()) };
    /// Nesting depth of `CompileProfileGuard` (inside `RibCompiler::compile`).
    static COMPILE_SESSION_DEPTH: Cell<u32> = const { Cell::new(0) };
}

pub(crate) fn rib_profile_enabled() -> bool {
    PROFILE_INIT.call_once(|| {
        let on = std::env::var_os("RIB_PROFILE").is_some_and(|v| {
            let s = v.to_string_lossy();
            s == "1"
                || s.eq_ignore_ascii_case("true")
                || s.eq_ignore_ascii_case("yes")
                || s.eq_ignore_ascii_case("on")
        });
        PROFILE_ENABLED.store(on, Ordering::Relaxed);
    });
    PROFILE_ENABLED.load(Ordering::Relaxed)
}

fn record_phase(label: &'static str, elapsed: Duration) {
    PROFILE_ENTRIES.with(|e| e.borrow_mut().push((label, elapsed)));
}

/// These duplicate the sum of finer-grained rows (or subsume `infer_types`).
const EXCLUDE_FROM_SUMMARY: &[&str] = &[
    "compile: RibCompiler.infer_types InferredExpr::from_expr",
    "compile: RibCompiler.compile infer_types (total)",
];

fn format_duration(d: Duration) -> String {
    let secs = d.as_secs_f64();
    if secs >= 10.0 {
        format!("{:.1}s", secs)
    } else if secs >= 1.0 {
        format!("{:.2}s", secs)
    } else if d.as_millis() >= 1 {
        format!("{}ms", d.as_millis())
    } else {
        format!("{}µs", d.as_micros())
    }
}

fn print_summary_table(wall: Duration, title: &str) {
    let mut rows: Vec<(&'static str, Duration)> =
        PROFILE_ENTRIES.with(|e| e.borrow_mut().drain(..).collect());
    rows.retain(|(label, _)| !EXCLUDE_FROM_SUMMARY.contains(label));

    if rows.is_empty() {
        return;
    }

    rows.sort_by_key(|row| Reverse(row.1));

    let s = Styles::detect();
    let prefix = format!("{}[rib-profile]{} ", s.tag(), s.r());

    let wall_ms = wall.as_secs_f64() * 1000.0;
    let w_label = 58usize;
    let w_time = 10usize;
    let w_pct = 8usize;

    let r = s.r();
    eprintln!();
    eprintln!(
        "{prefix}{ti}{title}{r}",
        prefix = prefix,
        ti = s.title(),
        title = title,
        r = r
    );
    eprintln!(
        "{prefix}{h}{phase:<wl$}{r} {h}{time:>wt$}{r} {h}{pct_head:>wp$}{r}",
        prefix = prefix,
        h = s.header(),
        phase = "phase",
        time = "time",
        pct_head = "% wall",
        r = r,
        wl = w_label,
        wt = w_time,
        wp = w_pct
    );
    let dash_l = "-".repeat(w_label);
    let dash_t = "-".repeat(w_time);
    let dash_p = "-".repeat(w_pct);
    let sep = s.sep();
    eprintln!(
        "{prefix}{sep}{dash_l}{r} {sep}{dash_t}{r} {sep}{dash_p}{r}",
        prefix = prefix,
        sep = sep,
        dash_l = dash_l,
        dash_t = dash_t,
        dash_p = dash_p,
        r = r
    );

    let mut listed_sum = Duration::ZERO;
    for (label, elapsed) in &rows {
        listed_sum += *elapsed;
        let pct = if wall_ms > 0.0 {
            (elapsed.as_secs_f64() * 1000.0 / wall_ms) * 100.0
        } else {
            0.0
        };
        let label_short = if label.len() > w_label {
            &label[..w_label]
        } else {
            label
        };
        let phase_style = if label.starts_with("infer_types:") {
            s.phase_infer()
        } else if label.starts_with("compile:") {
            s.phase_compile()
        } else {
            ""
        };
        let time_s = format_duration(*elapsed);
        eprintln!(
            "{prefix}{ps}{label:<wl$}{r} {tc}{time:>wt$}{r} {pc}{pct:>wp$.1}%{r}",
            prefix = prefix,
            ps = phase_style,
            label = label_short,
            tc = s.time_col(),
            time = time_s,
            pc = s.pct(pct),
            pct = pct,
            r = r,
            wl = w_label,
            wt = w_time,
            wp = w_pct
        );
    }

    eprintln!(
        "{prefix}{sep}{dash_l}{r} {sep}{dash_t}{r} {sep}{dash_p}{r}",
        prefix = prefix,
        sep = sep,
        dash_l = dash_l,
        dash_t = dash_t,
        dash_p = dash_p,
        r = r
    );
    let sum_s = format_duration(listed_sum);
    eprintln!(
        "{prefix}{fl}{sum_lbl:<wl$}{r} {tc}{sum_s:>wt$}{r}",
        prefix = prefix,
        fl = s.footer_label(),
        sum_lbl = "(sum of rows above)",
        tc = s.time_col(),
        sum_s = sum_s,
        r = r,
        wl = w_label,
        wt = w_time
    );
    let gap = wall.saturating_sub(listed_sum);
    let gap_note = if listed_sum > wall {
        format!(
            "rows exceed wall by {}",
            format_duration(listed_sum.saturating_sub(wall))
        )
    } else {
        format!("uninstrumented gap ≈ {}", format_duration(gap))
    };
    let wall_s = format_duration(wall);
    eprintln!(
        "{prefix}{fl}{wall_lbl:<wl$}{r} {tc}{wall_s:>wt$}{r}  {fnote}({gap_note}){r}",
        prefix = prefix,
        fl = s.footer_label(),
        wall_lbl = "compile / session wall",
        tc = s.time_col(),
        wall_s = wall_s,
        fnote = s.footer_note(),
        gap_note = gap_note,
        r = r,
        wl = w_label,
        wt = w_time
    );
    eprintln!();
}

/// Wraps `RibCompiler::compile`: clears entries on outermost enter, prints table on exit.
pub(crate) struct CompileProfileGuard {
    start: Option<Instant>,
}

impl CompileProfileGuard {
    pub fn enter() -> Self {
        if !rib_profile_enabled() {
            return Self { start: None };
        }
        let outermost = COMPILE_SESSION_DEPTH.with(|c| {
            let d = c.get();
            c.set(d + 1);
            d == 0
        });
        if outermost {
            PROFILE_ENTRIES.with(|e| e.borrow_mut().clear());
        }
        Self {
            start: Some(Instant::now()),
        }
    }
}

impl Drop for CompileProfileGuard {
    fn drop(&mut self) {
        if !rib_profile_enabled() {
            return;
        }
        let wall = self.start.take().map(|s| s.elapsed());
        COMPILE_SESSION_DEPTH.with(|c| {
            let d = c.get();
            if d == 1 {
                if let Some(w) = wall {
                    print_summary_table(w, "compile phases (sorted by time; % of compile wall)");
                }
            }
            c.set(d.saturating_sub(1));
        });
    }
}

/// When `infer_types` is called without `compile`, still emit a summary.
pub(crate) struct InferOnlyProfileGuard {
    start: Option<Instant>,
    active: bool,
}

impl InferOnlyProfileGuard {
    pub fn new() -> Self {
        if !rib_profile_enabled() {
            return Self {
                start: None,
                active: false,
            };
        }
        let inside_compile = COMPILE_SESSION_DEPTH.with(|c| c.get() > 0);
        if inside_compile {
            return Self {
                start: None,
                active: false,
            };
        }
        PROFILE_ENTRIES.with(|e| e.borrow_mut().clear());
        Self {
            start: Some(Instant::now()),
            active: true,
        }
    }
}

impl Drop for InferOnlyProfileGuard {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        if let Some(start) = self.start.take() {
            print_summary_table(
                start.elapsed(),
                "infer_types phases (sorted by time; % of session wall)",
            );
        }
    }
}

/// On drop, records elapsed time if `RIB_PROFILE` is set.
///
/// When profiling is **off**, this does not call [`Instant::now()`] — only a cached atomic load
/// and an empty [`Drop`], so production builds stay effectively unchanged.
pub(crate) struct Scope {
    /// `Some` only when `RIB_PROFILE` is enabled for this process.
    inner: Option<(&'static str, Instant)>,
}

impl Scope {
    #[inline]
    pub fn new(label: &'static str) -> Self {
        Self {
            inner: rib_profile_enabled().then(|| (label, Instant::now())),
        }
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        if let Some((label, start)) = self.inner.take() {
            record_phase(label, start.elapsed());
        }
    }
}
