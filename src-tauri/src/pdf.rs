use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, State};

use crate::assessment::{AreaResult, Assessment};

const W: f64 = 842.0;
const H: f64 = 595.0;
const M: f64 = 40.0;

type Rgb = (f64, f64, f64);
const TEXT: Rgb = (0.12, 0.14, 0.16);
const MUTED: Rgb = (0.39, 0.42, 0.45);
const RULE: Rgb = (0.86, 0.85, 0.82);
const ACCENT: Rgb = (0.184, 0.435, 0.369);
const CEILING: Rgb = (0.831, 0.533, 0.165);
const FLOOR: Rgb = (0.247, 0.404, 0.690);

const WIDTHS: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278,
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556,
    1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778,
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556,
    333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556,
    556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];

const SHORT: [(&str, &str); 8] = [
    ("Körperbewusstsein & -pflege", "Körper"),
    ("Umgebungsbewusstsein", "Umgebung"),
    ("Sozial-emotionale Entwicklung", "Sozial-emotional"),
    ("Spieltätigkeit", "Spiel"),
    ("Sprache & Literacy", "Sprache"),
    ("Kognition", "Kognition"),
    ("Grobmotorik", "Grobmotorik"),
    ("Feinmotorik", "Feinmotorik"),
];

fn win_ansi(c: char) -> u8 {
    match c as u32 {
        0x20..=0x7e | 0xa0..=0xff => c as u32 as u8,
        _ => match c {
            '€' => 0x80,
            '‚' => 0x82,
            '„' => 0x84,
            '…' => 0x85,
            '‘' => 0x91,
            '’' => 0x92,
            '“' => 0x93,
            '”' => 0x94,
            '•' => 0x95,
            '–' => 0x96,
            '—' => 0x97,
            _ => b'?',
        },
    }
}

fn char_width(c: char) -> f64 {
    let base = match c {
        'ä' => 'a',
        'ö' => 'o',
        'ü' => 'u',
        'Ä' => 'A',
        'Ö' => 'O',
        'Ü' => 'U',
        other => other,
    };
    let w = match base as u32 {
        32..=126 => WIDTHS[base as usize - 32],
        _ => match base {
            'ß' => 611,
            '–' => 556,
            '·' | '„' | '“' | '”' => 333,
            _ => 556,
        },
    };
    w as f64
}

fn text_width(s: &str, size: f64, bold: bool) -> f64 {
    let raw: f64 = s.chars().map(char_width).sum::<f64>() * size / 1000.0;
    if bold {
        raw * 1.08
    } else {
        raw
    }
}

fn wrap(s: &str, width: f64, size: f64, bold: bool) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in s.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };
        if text_width(&candidate, size, bold) <= width {
            current = candidate;
            continue;
        }
        if !current.is_empty() {
            lines.push(std::mem::take(&mut current));
        }
        for c in word.chars() {
            let next = format!("{current}{c}");
            if !current.is_empty() && text_width(&next, size, bold) > width {
                lines.push(std::mem::take(&mut current));
                current.push(c);
            } else {
                current = next;
            }
        }
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }
    lines
}

pub fn fmt_num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.2}").replace('.', ",")
    }
}

#[derive(Default)]
struct Page {
    ops: Vec<u8>,
}

impl Page {
    fn raw(&mut self, s: &str) {
        self.ops.extend_from_slice(s.as_bytes());
    }

    fn fill(&mut self, c: Rgb) {
        self.raw(&format!("{:.3} {:.3} {:.3} rg\n", c.0, c.1, c.2));
    }

    fn stroke(&mut self, c: Rgb) {
        self.raw(&format!("{:.3} {:.3} {:.3} RG\n", c.0, c.1, c.2));
    }

    fn text_rot(&mut self, x: f64, y: f64, size: f64, bold: bool, color: Rgb, s: &str, angle: f64) {
        let (sin, cos) = angle.to_radians().sin_cos();
        let font = if bold { "F2" } else { "F1" };
        self.fill(color);
        self.raw(&format!(
            "BT /{font} {size:.2} Tf {cos:.4} {sin:.4} {:.4} {cos:.4} {x:.2} {:.2} Tm (",
            -sin,
            H - y
        ));
        for c in s.chars() {
            let b = win_ansi(c);
            if matches!(b, b'(' | b')' | b'\\') {
                self.ops.push(b'\\');
            }
            self.ops.push(b);
        }
        self.raw(") Tj ET\n");
    }

    fn text(&mut self, x: f64, y: f64, size: f64, bold: bool, color: Rgb, s: &str) {
        self.text_rot(x, y, size, bold, color, s, 0.0);
    }

    fn text_right(&mut self, x: f64, y: f64, size: f64, bold: bool, color: Rgb, s: &str) {
        let w = text_width(s, size, bold);
        self.text(x - w, y, size, bold, color, s);
    }

    fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, width: f64, color: Rgb, dashed: bool) {
        self.stroke(color);
        let dash = if dashed { "[5 4] 0 d" } else { "[] 0 d" };
        self.raw(&format!(
            "{width:.2} w {dash} {x1:.2} {:.2} m {x2:.2} {:.2} l S\n",
            H - y1,
            H - y2
        ));
    }

    fn polyline(&mut self, points: &[(f64, f64)], width: f64, color: Rgb) {
        if points.len() < 2 {
            return;
        }
        self.stroke(color);
        self.raw(&format!("{width:.2} w [] 0 d 1 j "));
        for (i, (x, y)) in points.iter().enumerate() {
            let op = if i == 0 { "m" } else { "l" };
            self.raw(&format!("{x:.2} {:.2} {op} ", H - y));
        }
        self.raw("S\n");
    }

    fn circle(&mut self, cx: f64, cy: f64, r: f64, color: Rgb) {
        let k = 0.5523 * r;
        let y = H - cy;
        self.fill(color);
        self.raw(&format!(
            "{:.2} {y:.2} m {:.2} {:.2} {:.2} {:.2} {cx:.2} {:.2} c {:.2} {:.2} {:.2} {:.2} {:.2} {y:.2} c {:.2} {:.2} {:.2} {:.2} {cx:.2} {:.2} c {:.2} {:.2} {:.2} {:.2} {:.2} {y:.2} c f\n",
            cx + r,
            cx + r, y + k, cx + k, y + r, y + r,
            cx - k, y + r, cx - r, y + k, cx - r,
            cx - r, y - k, cx - k, y - r, y - r,
            cx + k, y - r, cx + r, y - k, cx + r,
        ));
    }
}

fn short_name(area: &str) -> &str {
    SHORT
        .iter()
        .find(|(long, _)| *long == area)
        .map(|(_, short)| *short)
        .unwrap_or(area)
}

fn phase_values(r: &AreaResult) -> String {
    r.phases
        .iter()
        .map(|p| {
            let v = p.value.map(fmt_num).unwrap_or_else(|| "–".into());
            format!("{}: {v}", p.phase)
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

fn list(items: &[String]) -> String {
    if items.is_empty() {
        "–".into()
    } else {
        items.join(", ")
    }
}

fn header(page: &mut Page, title: &str, age_months: u32, date: &str) {
    page.text(M, 54.0, 20.0, true, TEXT, title);
    page.text(M, 72.0, 9.5, false, MUTED, "Kuno Bellers Entwicklungstabelle 0–9");
    page.text_right(W - M, 54.0, 11.0, true, TEXT, &format!("Alter: {age_months} Monate"));
    page.text_right(W - M, 72.0, 9.5, false, MUTED, &format!("Erstellt am {date}"));
    page.line(M, 86.0, W - M, 86.0, 0.8, RULE, false);
}

fn profile_page(age_months: u32, date: &str, results: &[AreaResult], median: f64) -> Page {
    let mut page = Page::default();
    header(&mut page, "Entwicklungsprofil", age_months, date);

    let (l, r, t, b) = (70.0, 470.0, 128.0, 390.0);
    page.text(M, 112.0, 11.0, true, TEXT, "Profil");
    let n = results.len().max(1) as f64;
    let x = |i: usize| l + (i as f64 + 0.5) * (r - l) / n;
    let y = |v: f64| b - v / 18.0 * (b - t);

    for v in (0..=18).step_by(2) {
        let yy = y(v as f64);
        page.line(l, yy, r, yy, 0.5, RULE, false);
        page.text_right(l - 6.0, yy + 3.0, 8.0, false, MUTED, &v.to_string());
    }
    let angle: f64 = 35.0;
    for (i, res) in results.iter().enumerate() {
        let label = short_name(&res.area);
        let w = text_width(label, 8.5, false);
        let (sin, cos) = angle.to_radians().sin_cos();
        page.text_rot(x(i) + 3.0 - w * cos, b + 14.0 + w * sin, 8.5, false, MUTED, label, angle);
    }

    page.line(l, y(median), r, y(median), 1.2, MUTED, true);
    for (key, color, width, radius) in [
        ("ceiling", CEILING, 1.6, 3.0),
        ("floor", FLOOR, 1.6, 3.0),
        ("total", ACCENT, 2.4, 3.8),
    ] {
        let value = |res: &AreaResult| match key {
            "ceiling" => res.ceiling as f64,
            "floor" => res.floor as f64,
            _ => res.total,
        };
        let points: Vec<(f64, f64)> = results.iter().enumerate().map(|(i, res)| (x(i), y(value(res)))).collect();
        page.polyline(&points, width, color);
        for (px, py) in points {
            page.circle(px, py, radius, color);
        }
    }

    let mut lx = l;
    let ly = 492.0;
    for (label, color, dashed) in [
        ("Decke".to_string(), CEILING, false),
        ("Gesamtwert".to_string(), ACCENT, false),
        ("Boden".to_string(), FLOOR, false),
        (format!("Median {}", fmt_num(median)), MUTED, true),
    ] {
        page.line(lx, ly - 3.0, lx + 16.0, ly - 3.0, 2.0, color, dashed);
        page.text(lx + 21.0, ly, 9.0, false, TEXT, &label);
        lx += 21.0 + text_width(&label, 9.0, false) + 22.0;
    }

    let x0 = 506.0;
    let cols = [x0, x0 + 162.0, x0 + 210.0, x0 + 258.0];
    page.text(x0, 112.0, 11.0, true, TEXT, "Werte");
    page.text(cols[0], 134.0, 8.5, true, MUTED, "Bereich");
    page.text(cols[1], 134.0, 8.5, true, MUTED, "Boden");
    page.text(cols[2], 134.0, 8.5, true, MUTED, "Decke");
    page.text(cols[3], 134.0, 8.5, true, MUTED, "Gesamt");
    page.line(x0, 141.0, W - M, 141.0, 0.8, RULE, false);

    let mut row = 158.0;
    for res in results {
        let lines = wrap(&res.area, 150.0, 9.0, false);
        for (i, line) in lines.iter().enumerate() {
            page.text(cols[0], row + i as f64 * 11.0, 9.0, false, TEXT, line);
        }
        page.text(cols[1], row, 10.0, false, TEXT, &res.floor.to_string());
        page.text(cols[2], row, 10.0, false, TEXT, &res.ceiling.to_string());
        page.text(cols[3], row, 10.0, true, ACCENT, &fmt_num(res.total));
        let h = (lines.len() as f64 * 11.0).max(14.0) + 8.0;
        page.line(x0, row + h - 10.0, W - M, row + h - 10.0, 0.5, RULE, false);
        row += h;
    }
    page.text(cols[0], row + 2.0, 9.5, true, TEXT, "Median der Gesamtwerte");
    page.text(cols[3], row + 2.0, 10.0, true, TEXT, &fmt_num(median));

    page
}

fn answer_pages(age_months: u32, date: &str, results: &[AreaResult]) -> Vec<Page> {
    let cols: [(&str, f64); 6] = [
        ("Bereich", 128.0),
        ("Tut es", 150.0),
        ("Tut es teilweise", 118.0),
        ("Tut es nicht", 150.0),
        ("Weiß ich nicht", 100.0),
        ("Phasenwerte", 116.0),
    ];
    let size = 8.5;
    let lh = 11.0;
    let bottom = H - 48.0;

    let start_page = |pages: &mut Vec<Page>| -> f64 {
        let mut page = Page::default();
        header(&mut page, "Antworten je Bereich", age_months, date);
        let mut x = M;
        for (title, w) in cols {
            page.text(x, 108.0, size, true, MUTED, title);
            x += w;
        }
        page.line(M, 115.0, W - M, 115.0, 0.8, RULE, false);
        pages.push(page);
        130.0
    };

    let mut pages = Vec::new();
    let mut y = start_page(&mut pages);

    for res in results {
        let cells = [
            res.area.clone(),
            list(&res.table.ja),
            list(&res.table.teilweise),
            list(&res.table.nein),
            list(&res.table.weiss_nicht),
            phase_values(res),
        ];
        let wrapped: Vec<Vec<String>> = cells
            .iter()
            .zip(cols.iter())
            .enumerate()
            .map(|(i, (text, (_, w)))| wrap(text, w - 10.0, size, i == 0))
            .collect();
        let rows = wrapped.iter().map(|w| w.len()).max().unwrap_or(1);
        let h = rows as f64 * lh + 10.0;
        if y + h > bottom {
            y = start_page(&mut pages);
        }
        let page = pages.last_mut().unwrap();
        let mut x = M;
        for (i, (lines, (_, w))) in wrapped.iter().zip(cols.iter()).enumerate() {
            for (j, line) in lines.iter().enumerate() {
                page.text(x, y + j as f64 * lh, size, i == 0, TEXT, line);
            }
            x += w;
        }
        page.line(M, y + h - lh, W - M, y + h - lh, 0.5, RULE, false);
        y += h;
    }
    pages
}

fn assemble(pages: Vec<Page>) -> Vec<u8> {
    let n = pages.len();
    let mut objects: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        format!(
            "<< /Type /Pages /Kids [{}] /Count {n} >>",
            (0..n).map(|i| format!("{} 0 R", 5 + 2 * i)).collect::<Vec<_>>().join(" ")
        )
        .into_bytes(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>".to_vec(),
    ];
    for (i, page) in pages.into_iter().enumerate() {
        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {W} {H}] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {} 0 R >>",
                6 + 2 * i
            )
            .into_bytes(),
        );
        let mut content = format!("<< /Length {} >>\nstream\n", page.ops.len()).into_bytes();
        content.extend_from_slice(&page.ops);
        content.extend_from_slice(b"\nendstream");
        objects.push(content);
    }

    let mut out = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for offset in offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes(),
    );
    out
}

pub fn report(age_months: u32, date: &str, results: &[AreaResult], median: f64) -> Vec<u8> {
    let mut pages = vec![profile_page(age_months, date, results, median)];
    pages.extend(answer_pages(age_months, date, results));
    let total = pages.len();
    for (i, page) in pages.iter_mut().enumerate() {
        page.text(M, H - 22.0, 7.5, false, MUTED, "Kuno Bellers Entwicklungstabelle 0–9 © Simone Beller (2016) · erstellt mit Bellreader");
        page.text_right(W - M, H - 22.0, 7.5, false, MUTED, &format!("Seite {} von {total}", i + 1));
    }
    assemble(pages)
}

pub fn civil_date(unix_seconds: i64) -> (i64, u32, u32) {
    let z = unix_seconds.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    (y, m, d)
}

fn free_path(dir: &Path, base: &str) -> PathBuf {
    let first = dir.join(format!("{base}.pdf"));
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|i| dir.join(format!("{base} ({i}).pdf")))
        .find(|p| !p.exists())
        .unwrap()
}

#[tauri::command]
pub fn export_pdf(app: AppHandle, state: State<'_, Mutex<Assessment>>) -> Result<String, String> {
    let (age, results, median) = {
        let assessment = state.lock().map_err(|e| e.to_string())?;
        let (results, median) = assessment
            .finished()
            .ok_or("Die Beobachtung ist noch nicht abgeschlossen.")?;
        (assessment.age_months(), results, median)
    };
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, m, d) = civil_date(seconds + 3_600);
    let bytes = report(age, &format!("{d:02}.{m:02}.{y}"), &results, median);
    let dir = app.path().download_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = free_path(&dir, &format!("Bellreader_{y}-{m:02}-{d:02}_{age}-Monate"));
    std::fs::write(&path, bytes).map_err(|e| format!("PDF konnte nicht gespeichert werden: {e}"))?;
    Ok(path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assessment::{Answer, Step};

    fn finished(age: u32) -> (Vec<AreaResult>, f64) {
        let mut a = Assessment::new(age);
        let mut step = a.advance();
        while let Step::Question { phase, condition_index, .. } = step {
            let answer = match phase {
                1..=4 => Answer::Ja,
                5 if condition_index % 2 == 0 => Answer::Teilweise,
                5 | 6 => Answer::WeissNicht,
                _ => Answer::Nein,
            };
            step = a.answer(answer).unwrap();
        }
        a.finished().unwrap()
    }

    #[test]
    fn dates() {
        assert_eq!(civil_date(0), (1970, 1, 1));
        assert_eq!(civil_date(1_794_787_200), (2026, 11, 16));
        assert_eq!(civil_date(951_782_400), (2000, 2, 29));
    }

    #[test]
    fn numbers_use_german_format() {
        assert_eq!(fmt_num(6.0), "6");
        assert_eq!(fmt_num(4.7), "4,70");
    }

    #[test]
    fn wraps_long_text() {
        let lines = wrap("1.1, 1.2, 1.3, 1.4, 1.5, 2.1, 2.2, 2.3, 2.4, 2.5, 3.1, 3.2", 60.0, 8.5, false);
        assert!(lines.len() > 1);
        assert!(lines.iter().all(|l| text_width(l, 8.5, false) <= 60.0));
    }

    #[test]
    fn report_is_a_valid_pdf() {
        let (results, median) = finished(40);
        let pdf = report(40, "16.11.2026", &results, median);
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.ends_with(b"%%EOF\n"));
        let text = String::from_utf8_lossy(&pdf);
        let xref: usize = text.rsplit("startxref\n").next().unwrap().lines().next().unwrap().parse().unwrap();
        assert!(pdf[xref..].starts_with(b"xref"));
        for (i, line) in text[xref..].lines().skip(3).take_while(|l| l.ends_with(" n ")).enumerate() {
            let offset: usize = line[..10].parse().unwrap();
            assert!(pdf[offset..].starts_with(format!("{} 0 obj", i + 1).as_bytes()));
        }
        if let Ok(path) = std::env::var("BELLREADER_PDF_OUT") {
            std::fs::write(path, &pdf).unwrap();
        }
    }
}
