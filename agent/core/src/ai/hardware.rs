//! Hardware detection and AI model recommendations (DATA_MODEL.md 12.2).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vram_gb: Option<f64>,
    pub ram_gb: f64,
}

impl HardwareInfo {
    /// Recommended model choice: "standard" (Qwen3.5 4B) or "light" (Qwen3.5 2B).
    pub fn recommended_model(&self) -> &'static str {
        recommend_model(self.vram_gb, self.ram_gb)
    }

    /// The extended model (Qwen3.5 9B) is offered from 16 GB of RAM or 8 GB of video memory (T27.7).
    pub fn extended_ok(&self) -> bool {
        self.ram_gb >= 16.0 || self.vram_gb.is_some_and(|v| v >= 8.0)
    }

    /// Speech recognition goes to the video card: NVIDIA (the only ready-made GPU build of whisper.cpp for Windows is
    /// cuBLAS) with at least 2 GB of video memory.
    pub fn whisper_gpu(&self) -> bool {
        self.gpu.as_deref().is_some_and(|g| g.to_ascii_lowercase().contains("nvidia")) && self.vram_gb.is_some_and(|v| v >= 2.0)
    }

    /// Formats the hardware summary line for the settings window.
    /// E.g. "This PC: NVIDIA GeForce RTX 2060 SUPER (8 GB), 15.8 GB RAM — standard model"
    pub fn summary_string(&self) -> String {
        let model = self.recommended_model();
        let mut parts = Vec::new();

        if let Some(ref gpu) = self.gpu {
            if let Some(vram) = self.vram_gb {
                parts.push(format!("{} ({:.0} GB)", gpu, vram));
            } else {
                parts.push(gpu.clone());
            }
        }

        parts.push(format!("{:.1} GB RAM", self.ram_gb));

        format!("This PC: {} — {} model", parts.join(", "), model)
    }
}

/// Recommendation rule (DATA_MODEL.md 12.2):
/// - `llm-standard` (4B) if VRAM >= 6 GB, or without such a GPU, if RAM >= 16 GB.
/// - Otherwise `llm-light` (2B).
pub fn recommend_model(vram_gb: Option<f64>, ram_gb: f64) -> &'static str {
    if let Some(vram) = vram_gb {
        if vram >= 6.0 {
            return "standard";
        }
    }
    if ram_gb >= 16.0 {
        "standard"
    } else {
        "light"
    }
}

/// Detects GPU name, VRAM (via DXGI on Windows), and system RAM.
pub fn detect_hardware() -> HardwareInfo {
    let mut total_bytes = 0u64;
    if let Ok(mut sys) = sysinfo::System::new() {
        sys.refresh_memory();
        total_bytes = sys.total_memory();
    }
    let ram_gb = (total_bytes as f64 / (1024.0 * 1024.0 * 1024.0) * 10.0).round() / 10.0;

    let (gpu, vram_gb) = detect_gpu();

    HardwareInfo {
        gpu,
        vram_gb,
        ram_gb,
    }
}

#[cfg(windows)]
fn detect_gpu() -> (Option<String>, Option<f64>) {
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};

    unsafe {
        if let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() {
            let mut best_name: Option<String> = None;
            let mut max_vram = 0usize;
            let mut idx = 0;

            while let Ok(adapter) = factory.EnumAdapters1(idx) {
                idx += 1;
                if let Ok(desc) = adapter.GetDesc1() {
                    let vram = desc.DedicatedVideoMemory;
                    if vram > max_vram {
                        max_vram = vram;
                        let name = String::from_utf16_lossy(&desc.Description)
                            .trim_matches('\0')
                            .trim()
                            .to_string();
                        best_name = Some(name);
                    }
                }
            }

            if let Some(name) = best_name {
                let vram_gb = (max_vram as f64 / (1024.0 * 1024.0 * 1024.0) * 10.0).round() / 10.0;
                return (Some(name), Some(vram_gb));
            }
        }
    }
    (None, None)
}

#[cfg(not(windows))]
fn detect_gpu() -> (Option<String>, Option<f64>) {
    (None, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recommend_model_with_sufficient_vram() {
        // VRAM >= 6 GB recommends standard regardless of RAM
        expect_eq(recommend_model(Some(6.0), 8.0), "standard");
        expect_eq(recommend_model(Some(8.0), 8.0), "standard");
        expect_eq(recommend_model(Some(12.0), 32.0), "standard");
    }

    #[test]
    fn test_recommend_model_with_insufficient_vram_but_sufficient_ram() {
        // VRAM < 6 GB, but RAM >= 16 GB recommends standard
        expect_eq(recommend_model(Some(4.0), 16.0), "standard");
        expect_eq(recommend_model(Some(4.0), 32.0), "standard");
        expect_eq(recommend_model(Some(2.0), 16.0), "standard");
    }

    #[test]
    fn test_recommend_model_weak_pc() {
        // VRAM < 6 GB and RAM < 16 GB recommends light
        expect_eq(recommend_model(Some(4.0), 8.0), "light");
        expect_eq(recommend_model(Some(2.0), 12.0), "light");
        expect_eq(recommend_model(Some(0.0), 8.0), "light");
    }

    #[test]
    fn test_recommend_model_no_gpu() {
        // No dedicated GPU
        expect_eq(recommend_model(None, 16.0), "standard");
        expect_eq(recommend_model(None, 32.0), "standard");
        expect_eq(recommend_model(None, 8.0), "light");
        expect_eq(recommend_model(None, 15.9), "light");
    }

    #[test]
    fn test_summary_string() {
        let hw = HardwareInfo {
            gpu: Some("NVIDIA GeForce RTX 2060 SUPER".to_string()),
            vram_gb: Some(8.0),
            ram_gb: 15.8,
        };
        assert_eq!(
            hw.summary_string(),
            "This PC: NVIDIA GeForce RTX 2060 SUPER (8 GB), 15.8 GB RAM — standard model"
        );

        let weak_hw = HardwareInfo {
            gpu: None,
            vram_gb: None,
            ram_gb: 8.0,
        };
        assert_eq!(weak_hw.summary_string(), "This PC: 8.0 GB RAM — light model");
    }

    fn expect_eq(actual: &str, expected: &str) {
        assert_eq!(actual, expected);
    }
}
