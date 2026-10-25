// REOX Profiler - Report Generation
// Formats profiling data for display

#![allow(dead_code, unused_imports)]

use super::{ProfilingSummary, OutputFormat};
use std::fmt::Arguments;

/// Append one formatted line. Writing into a String cannot fail, so this
/// keeps the report builders free of error handling.
fn line(out: &mut String, args: Arguments<'_>) {
    out.push_str(&args.to_string());
    out.push('\n');
}

/// Generate a text report
pub fn generate_text_report(summary: &ProfilingSummary) -> String {
    let mut output = String::new();

    output.push('\n');
    line(&mut output, format_args!("=== REOX Profiling Report ==="));
    line(&mut output, format_args!("Total execution time: {:.3}ms", 
             summary.total_time.as_secs_f64() * 1000.0));
    output.push('\n');

    line(&mut output, format_args!("{:<30} {:>10} {:>12} {:>12} {:>12}",
             "Function", "Calls", "Total (ms)", "Self (ms)", "Avg (us)"));
    line(&mut output, format_args!("{:-<30} {:->10} {:->12} {:->12} {:->12}",
             "", "", "", "", ""));

    for func in &summary.functions {
        let total_ms = func.total_time.as_secs_f64() * 1000.0;
        let self_ms = func.self_time.as_secs_f64() * 1000.0;
        let avg_us = func.avg_time.as_secs_f64() * 1_000_000.0;

        line(&mut output, format_args!("{:<30} {:>10} {:>12.3} {:>12.3} {:>12.3}",
                 truncate(&func.name, 30),
                 func.call_count,
                 total_ms,
                 self_ms,
                 avg_us));
    }

    if summary.total_allocations > 0 {
        output.push('\n');
        line(&mut output, format_args!("Memory: {} allocations, {} bytes total",
                 summary.total_allocations,
                 summary.total_bytes_allocated));
    }

    output.push('\n');
    line(&mut output, format_args!("============================="));

    output
}

/// Generate a JSON report
pub fn generate_json_report(summary: &ProfilingSummary) -> String {
    let mut output = String::new();

    line(&mut output, format_args!("{{"));
    line(&mut output, format_args!("  \"total_time_ms\": {:.3},",
             summary.total_time.as_secs_f64() * 1000.0));
    line(&mut output, format_args!("  \"functions\": ["));

    for (i, func) in summary.functions.iter().enumerate() {
        let comma = if i + 1 < summary.functions.len() { "," } else { "" };
        line(&mut output, format_args!("    {{"));
        line(&mut output, format_args!("      \"name\": \"{}\",", func.name));
        line(&mut output, format_args!("      \"calls\": {},", func.call_count));
        line(&mut output, format_args!("      \"total_ms\": {:.3},", 
                 func.total_time.as_secs_f64() * 1000.0));
        line(&mut output, format_args!("      \"avg_us\": {:.3}",
                 func.avg_time.as_secs_f64() * 1_000_000.0));
        line(&mut output, format_args!("    }}{}", comma));
    }

    line(&mut output, format_args!("  ],"));
    line(&mut output, format_args!("  \"memory\": {{"));
    line(&mut output, format_args!("    \"allocations\": {},", summary.total_allocations));
    line(&mut output, format_args!("    \"bytes\": {}", summary.total_bytes_allocated));
    line(&mut output, format_args!("  }}"));
    line(&mut output, format_args!("}}"));

    output
}

/// Generate flamegraph-compatible output (folded stacks)
pub fn generate_flamegraph_output(summary: &ProfilingSummary) -> String {
    let mut output = String::new();

    // Flamegraph format: stack;frame count
    for func in &summary.functions {
        let samples = (func.total_time.as_nanos() / 1_000_000) as u64; // 1ms = 1 sample
        if samples > 0 {
            line(&mut output, format_args!("{} {}", func.name, samples));
        }
    }

    output
}

/// Format report based on output format
pub fn format_report(summary: &ProfilingSummary, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => generate_text_report(summary),
        OutputFormat::Json => generate_json_report(summary),
        OutputFormat::Flamegraph => generate_flamegraph_output(summary),
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let keep = max_len.saturating_sub(3);
        format!("{}...", s.chars().take(keep).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiler::FunctionStats;
    use std::time::Duration;

    #[test]
    fn test_text_report() {
        let summary = ProfilingSummary {
            total_time: Duration::from_millis(100),
            functions: vec![
                FunctionStats {
                    name: "main".to_string(),
                    call_count: 1,
                    total_time: Duration::from_millis(100),
                    self_time: Duration::from_millis(50),
                    min_time: Duration::from_millis(100),
                    max_time: Duration::from_millis(100),
                    avg_time: Duration::from_millis(100),
                },
            ],
            total_allocations: 10,
            total_bytes_allocated: 1024,
        };

        let report = generate_text_report(&summary);
        assert!(report.contains("main"));
        assert!(report.contains("100.000"));
    }

    #[test]
    fn json_report_handles_no_functions() {
        let summary = ProfilingSummary {
            total_time: Duration::from_millis(1),
            functions: vec![],
            total_allocations: 0,
            total_bytes_allocated: 0,
        };
        let report = generate_json_report(&summary);
        assert!(report.contains("\"functions\": [\n  ],"));
    }

    #[test]
    fn truncate_respects_char_boundaries() {
        assert_eq!(truncate("héllo wörld, this is long", 10), "héllo w...");
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("abcdef", 2), "...");
    }

    #[test]
    fn text_report_truncates_long_names() {
        let summary = ProfilingSummary {
            total_time: Duration::from_millis(1),
            functions: vec![FunctionStats { name: "x".repeat(50), ..FunctionStats::default() }],
            total_allocations: 0,
            total_bytes_allocated: 0,
        };
        let report = generate_text_report(&summary);
        assert!(report.contains(&format!("{}...", "x".repeat(27))));
    }
}
