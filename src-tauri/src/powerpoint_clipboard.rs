use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::{
    env, fs,
    io::ErrorKind,
    path::PathBuf,
    process::{self, Command},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const SLIDE_WIDTH_POINTS: f64 = 960.0;
const SLIDE_HEIGHT_POINTS: f64 = 540.0;
const SLIDE_MARGIN_POINTS: f64 = 36.0;
static NEXT_CLIPBOARD_TEMP_DIR_ID: AtomicU64 = AtomicU64::new(0);

const COPY_TO_POWERPOINT_CLIPBOARD_SCRIPT: &str = r#"
param(
  [Parameter(Mandatory = $true)]
  [string]$SvgPath,

  [Parameter(Mandatory = $true)]
  [double]$Left,

  [Parameter(Mandatory = $true)]
  [double]$Top,

  [Parameter(Mandatory = $true)]
  [double]$Width,

  [Parameter(Mandatory = $true)]
  [double]$Height,

  [Parameter(Mandatory = $true)]
  [double]$SlideWidth,

  [Parameter(Mandatory = $true)]
  [double]$SlideHeight
)

$ErrorActionPreference = 'Stop'

$powerPoint = $null
$presentation = $null

try {
  try {
    $powerPoint = New-Object -ComObject PowerPoint.Application
  }
  catch {
    throw 'Microsoft PowerPoint must be installed to copy Mermaid diagrams as PowerPoint slides.'
  }

  $powerPoint.Visible = -1
  $presentation = $powerPoint.Presentations.Add()
  $presentation.PageSetup.SlideWidth = $SlideWidth
  $presentation.PageSetup.SlideHeight = $SlideHeight

  $slide = $presentation.Slides.Add(1, 12)
  $background = $slide.Shapes.AddShape(1, 0, 0, $SlideWidth, $SlideHeight)
  $background.Fill.Solid() | Out-Null
  $background.Fill.ForeColor.RGB = 0
  $background.Line.Visible = 0

  $shape = $slide.Shapes.AddPicture($SvgPath, 0, -1, $Left, $Top, $Width, $Height)
  $shape.Select() | Out-Null
  Start-Sleep -Milliseconds 500

  $shell = New-Object -ComObject WScript.Shell
  $null = $shell.AppActivate('PowerPoint')
  Start-Sleep -Milliseconds 500

  $confirmJob = Start-Job -ScriptBlock {
    $wshell = New-Object -ComObject WScript.Shell
    1..20 | ForEach-Object {
      Start-Sleep -Milliseconds 500
      $null = $wshell.SendKeys('{ENTER}')
    }
  }

  try {
    $powerPoint.CommandBars.ExecuteMso('SVGEdit') | Out-Null
    Receive-Job $confirmJob -Wait | Out-Null
    Start-Sleep -Seconds 2
  }
  finally {
    Remove-Job $confirmJob -Force -ErrorAction SilentlyContinue
  }

  if ($slide.Shapes.Count -le 2) {
    throw 'PowerPoint did not convert the Mermaid SVG into drawing objects.'
  }

  $slide.Copy()
}
finally {
  if ($presentation) {
    try {
      $presentation.Close()
    }
    catch {
    }
  }

  if ($powerPoint) {
    try {
      $powerPoint.Quit()
    }
    catch {
    }
  }

  [GC]::Collect()
  [GC]::WaitForPendingFinalizers()
}
"#;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MermaidClipboardDiagram {
    pub svg: String,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TargetBounds {
    left: f64,
    top: f64,
    width: f64,
    height: f64,
}

pub fn copy_mermaid_diagram_as_powerpoint(diagram: &MermaidClipboardDiagram) -> Result<()> {
    validate_diagram(diagram)?;

    let bounds = compute_target_bounds(diagram.width, diagram.height)?;
    let temp_dir = ClipboardTempDir::create()?;
    let svg_path = temp_dir.path.join("diagram.svg");
    let script_path = temp_dir.path.join("copy-mermaid-to-powerpoint.ps1");

    fs::write(&svg_path, diagram.svg.as_bytes())
        .with_context(|| format!("Failed to write {}", svg_path.display()))?;
    fs::write(&script_path, COPY_TO_POWERPOINT_CLIPBOARD_SCRIPT)
        .with_context(|| format!("Failed to write {}", script_path.display()))?;

    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(&script_path)
        .arg("-SvgPath")
        .arg(&svg_path)
        .arg("-Left")
        .arg(bounds.left.to_string())
        .arg("-Top")
        .arg(bounds.top.to_string())
        .arg("-Width")
        .arg(bounds.width.to_string())
        .arg("-Height")
        .arg(bounds.height.to_string())
        .arg("-SlideWidth")
        .arg(SLIDE_WIDTH_POINTS.to_string())
        .arg("-SlideHeight")
        .arg(SLIDE_HEIGHT_POINTS.to_string())
        .output()
        .context("Failed to launch PowerPoint clipboard automation")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let details = stderr.trim();
        let fallback = stdout.trim();
        let message = if !details.is_empty() {
            details
        } else if !fallback.is_empty() {
            fallback
        } else {
            "PowerPoint clipboard export failed."
        };
        bail!("{message}");
    }

    Ok(())
}

fn validate_diagram(diagram: &MermaidClipboardDiagram) -> Result<()> {
    if diagram.svg.trim().is_empty() {
        bail!("No Mermaid diagram is available to copy.");
    }

    if !looks_like_svg_document(&diagram.svg) {
        bail!("Mermaid diagram SVG is invalid.");
    }

    if !valid_dimension(diagram.width) || !valid_dimension(diagram.height) {
        bail!("Mermaid diagram does not have a valid size.");
    }

    Ok(())
}

fn looks_like_svg_document(svg: &str) -> bool {
    let document = svg.trim_start();
    let document = if document
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("<?xml"))
    {
        let Some(end) = document.find("?>") else {
            return false;
        };
        document[end + 2..].trim_start()
    } else {
        document
    };

    if !document
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("<svg"))
    {
        return false;
    }

    let suffix = &document[4..];
    suffix.starts_with("/>")
        || suffix
            .chars()
            .next()
            .is_some_and(|next| next == '>' || next.is_ascii_whitespace())
}

fn compute_target_bounds(source_width: f64, source_height: f64) -> Result<TargetBounds> {
    if !valid_dimension(source_width) || !valid_dimension(source_height) {
        bail!("Mermaid diagram dimensions must be positive.");
    }

    let fit_width = SLIDE_WIDTH_POINTS - (2.0 * SLIDE_MARGIN_POINTS);
    let fit_height = SLIDE_HEIGHT_POINTS - (2.0 * SLIDE_MARGIN_POINTS);
    let scale = (fit_width / source_width).min(fit_height / source_height);
    let width = (source_width * scale).max(1.0);
    let height = (source_height * scale).max(1.0);

    Ok(TargetBounds {
        left: (SLIDE_WIDTH_POINTS - width) / 2.0,
        top: (SLIDE_HEIGHT_POINTS - height) / 2.0,
        width,
        height,
    })
}

fn valid_dimension(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

struct ClipboardTempDir {
    path: PathBuf,
}

impl ClipboardTempDir {
    fn create() -> Result<Self> {
        let temp_root = env::temp_dir();

        for _ in 0..16 {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .context("System clock drifted before the Unix epoch")?
                .as_millis();
            let sequence = NEXT_CLIPBOARD_TEMP_DIR_ID.fetch_add(1, Ordering::Relaxed);
            let path = temp_root.join(format!(
                "mdv-powerpoint-clipboard-{timestamp}-{}-{sequence}",
                process::id()
            ));

            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("Failed to create {}", path.display()));
                }
            }
        }

        bail!("Failed to create a unique PowerPoint clipboard temp directory.")
    }
}

impl Drop for ClipboardTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ClipboardTempDir, MermaidClipboardDiagram, compute_target_bounds, validate_diagram,
    };
    use std::collections::HashSet;

    #[test]
    fn rejects_invalid_diagram_dimensions() {
        let diagram = MermaidClipboardDiagram {
            svg: String::from("<svg />"),
            width: 0.0,
            height: 120.0,
        };

        let error = validate_diagram(&diagram).expect_err("expected invalid Mermaid dimensions");
        assert_eq!(
            error.to_string(),
            "Mermaid diagram does not have a valid size."
        );
    }

    #[test]
    fn rejects_non_finite_diagram_dimensions() {
        let diagram = MermaidClipboardDiagram {
            svg: String::from("<svg />"),
            width: f64::NAN,
            height: 120.0,
        };

        let error = validate_diagram(&diagram).expect_err("expected non-finite Mermaid dimensions");
        assert_eq!(
            error.to_string(),
            "Mermaid diagram does not have a valid size."
        );

        assert!(compute_target_bounds(f64::INFINITY, 120.0).is_err());
    }

    #[test]
    fn rejects_non_svg_clipboard_payloads() {
        let diagram = MermaidClipboardDiagram {
            svg: String::from("<html><body>not an SVG</body></html>"),
            width: 120.0,
            height: 120.0,
        };

        let error = validate_diagram(&diagram).expect_err("expected non-SVG payload rejection");
        assert_eq!(error.to_string(), "Mermaid diagram SVG is invalid.");
    }

    #[test]
    fn rejects_svg_prefixes_that_are_not_svg_elements() {
        let diagram = MermaidClipboardDiagram {
            svg: String::from("<svg/onload=alert(1)>"),
            width: 120.0,
            height: 120.0,
        };

        let error = validate_diagram(&diagram).expect_err("expected malformed SVG rejection");
        assert_eq!(error.to_string(), "Mermaid diagram SVG is invalid.");
    }

    #[test]
    fn accepts_self_closing_svg_clipboard_payloads() {
        let diagram = MermaidClipboardDiagram {
            svg: String::from("<svg/>"),
            width: 120.0,
            height: 120.0,
        };

        validate_diagram(&diagram).expect("expected self-closing SVG payload to be valid");
    }

    #[test]
    fn accepts_svg_clipboard_payloads_with_xml_declaration() {
        let diagram = MermaidClipboardDiagram {
            svg: String::from("<?xml version=\"1.0\"?><svg width=\"120\" height=\"120\" />"),
            width: 120.0,
            height: 120.0,
        };

        validate_diagram(&diagram).expect("expected XML-prefixed SVG payload to be valid");
    }

    #[test]
    fn creates_distinct_temp_directories_for_rapid_exports() {
        let dirs = (0..64)
            .map(|_| ClipboardTempDir::create().expect("expected temp dir"))
            .collect::<Vec<_>>();
        let unique_paths = dirs
            .iter()
            .map(|dir| dir.path.clone())
            .collect::<HashSet<_>>();

        assert_eq!(unique_paths.len(), dirs.len());
    }

    #[test]
    fn scales_diagrams_to_fit_slide_bounds() {
        let bounds = compute_target_bounds(400.0, 200.0).expect("expected target bounds");

        assert!((bounds.width - 888.0).abs() < 0.01);
        assert!((bounds.height - 444.0).abs() < 0.01);
        assert!((bounds.left - 36.0).abs() < 0.01);
        assert!((bounds.top - 48.0).abs() < 0.01);
    }

    #[test]
    fn centers_tall_diagrams_on_slide() {
        let bounds = compute_target_bounds(200.0, 400.0).expect("expected target bounds");

        assert!((bounds.width - 234.0).abs() < 0.01);
        assert!((bounds.height - 468.0).abs() < 0.01);
        assert!((bounds.left - 363.0).abs() < 0.01);
        assert!((bounds.top - 36.0).abs() < 0.01);
    }
}
