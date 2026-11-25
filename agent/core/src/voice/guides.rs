//! Saved guides, "recipes" (T27.6): `%APPDATA%/Decka/guides/<id>.json` with the steps a person took, found again by the goal and the
//! program. A recipe is only a list of what to look for in the snapshot (role, name, automation id): it is followed without the model,
//! and every step is checked against the live window. Local only; nothing leaves the PC.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::matcher::{tokens, trigram_sim};
use super::ui_snapshot::{Element, Snapshot};

/// How alike two goals must be (0..1) to be the same one.
const GOAL_MATCH: f64 = 0.6;
/// A guide file larger than this is ignored (a damaged or foreign file).
const MAX_FILE: u64 = 256 * 1024;
pub const MAX_RECIPE_STEPS: usize = 30;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeStep {
    pub role: String,
    pub name: String,
    #[serde(rename = "automationId", default)]
    pub automation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recipe {
    pub id: String,
    pub goal: Vec<String>,
    /// Lower-case exe name of the program the recipe is for (`winword.exe`).
    pub process: String,
    pub steps: Vec<RecipeStep>,
    pub created: String,
    #[serde(default)]
    pub uses: u32,
}

impl RecipeStep {
    pub fn of(e: &Element) -> RecipeStep {
        RecipeStep { role: e.role.clone(), name: e.name.clone(), automation_id: e.automation_id.clone() }
    }

    /// The element of the live snapshot this step stands for: the automation id when there is one, else role and name.
    /// `None` when the window does not have it (then the model takes over).
    pub fn find<'a>(&self, snapshot: &'a Snapshot) -> Option<&'a Element> {
        if !self.automation_id.is_empty() {
            if let Some(e) = snapshot.elements.iter().find(|e| e.automation_id == self.automation_id && e.role == self.role) {
                return Some(e);
            }
        }
        snapshot.elements.iter().find(|e| e.role == self.role && !self.name.is_empty() && e.name.eq_ignore_ascii_case(&self.name))
    }
}

fn goal_score(a: &str, b: &str) -> f64 {
    let (ta, tb) = (tokens(a), tokens(b));
    if ta.is_empty() || tb.is_empty() {
        return 0.0;
    }
    if ta == tb {
        return 1.0;
    }
    trigram_sim(&ta.join(" "), &tb.join(" "))
}

/// A short file name for a goal: letters and digits only.
fn slug(goal: &str) -> String {
    let s: String = tokens(goal).join("-").chars().filter(|c| c.is_alphanumeric() || *c == '-').take(40).collect();
    if s.is_empty() { "guide".into() } else { s }
}

pub struct Guides {
    dir: PathBuf,
}

impl Guides {
    pub fn new(config_dir: &Path) -> Guides {
        Guides { dir: config_dir.join("guides") }
    }

    pub fn list(&self) -> Vec<Recipe> {
        let Ok(rd) = std::fs::read_dir(&self.dir) else { return Vec::new() };
        let mut out: Vec<Recipe> = rd
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "json") && e.metadata().is_ok_and(|m| m.len() <= MAX_FILE))
            .filter_map(|e| serde_json::from_slice(&std::fs::read(e.path()).ok()?).ok())
            .collect();
        out.sort_by(|a, b| a.created.cmp(&b.created).then(a.id.cmp(&b.id)));
        out
    }

    /// The recipe for this goal in this program, the best fit above the threshold.
    pub fn find(&self, goal: &str, process: &str) -> Option<Recipe> {
        self.list()
            .into_iter()
            .filter(|r| r.process.eq_ignore_ascii_case(process))
            .filter_map(|r| {
                let s = r.goal.iter().map(|g| goal_score(g, goal)).fold(0.0, f64::max);
                (s >= GOAL_MATCH).then_some((s, r))
            })
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
            .map(|x| x.1)
    }

    /// Saves a recipe; the same goal in the same program replaces the older one. Returns its id.
    pub fn save(&self, goal: &str, process: &str, steps: Vec<RecipeStep>, created: &str) -> Result<String, String> {
        if steps.is_empty() || steps.len() > MAX_RECIPE_STEPS {
            return Err("A guide needs 1 to 30 steps".into());
        }
        let id = format!("{}-{}", slug(process.to_lowercase().trim_end_matches(".exe")), slug(goal));
        let recipe = Recipe { id: id.clone(), goal: vec![goal.trim().to_string()], process: process.to_lowercase(), steps, created: created.to_string(), uses: 0 };
        self.write(&recipe)?;
        Ok(id)
    }

    fn write(&self, r: &Recipe) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec_pretty(r).map_err(|e| e.to_string())?;
        std::fs::write(self.dir.join(format!("{}.json", r.id)), bytes).map_err(|e| e.to_string())
    }

    pub fn used(&self, id: &str) {
        if let Some(mut r) = self.list().into_iter().find(|r| r.id == id) {
            r.uses += 1;
            let _ = self.write(&r);
        }
    }

    pub fn rename(&self, id: &str, goal: &str) -> Result<(), String> {
        let mut r = self.list().into_iter().find(|r| r.id == id).ok_or("No such guide")?;
        if goal.trim().is_empty() {
            return Err("The name is empty".into());
        }
        r.goal = vec![goal.trim().to_string()];
        self.write(&r)
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        // The id is a file name of this folder only.
        if id.is_empty() || id.contains(['/', '\\', '.']) {
            return Err("Bad guide id".into());
        }
        std::fs::remove_file(self.dir.join(format!("{id}.json"))).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::ui_snapshot::Rect;

    fn el(id: u32, role: &str, name: &str, aid: &str) -> Element {
        Element { id, role: role.into(), name: name.into(), automation_id: aid.into(), rect: Rect { x: 0, y: 0, width: 5, height: 5 }, parent_name: String::new() }
    }

    fn steps() -> Vec<RecipeStep> {
        vec![RecipeStep { role: "button".into(), name: "File".into(), automation_id: "FileTabButton".into() }, RecipeStep { role: "menuitem".into(), name: "Save As".into(), automation_id: String::new() }]
    }

    #[test]
    fn a_recipe_is_saved_found_by_goal_and_program_and_counted() {
        let dir = tempfile::tempdir().unwrap();
        let g = Guides::new(dir.path());
        let id = g.save("save as pdf", "WINWORD.EXE", steps(), "2026-11-26").unwrap();
        assert_eq!(id, "winword-save-as-pdf");
        assert_eq!(g.find("save as PDF", "winword.exe").unwrap().steps.len(), 2);
        assert!(g.find("save as pdf", "chrome.exe").is_none(), "another program");
        assert!(g.find("clear history", "winword.exe").is_none(), "another goal");
        g.used(&id);
        assert_eq!(g.list()[0].uses, 1);
        g.rename(&id, "export to pdf").unwrap();
        assert_eq!(g.list()[0].goal, vec!["export to pdf".to_string()]);
        g.delete(&id).unwrap();
        assert!(g.list().is_empty());
    }

    #[test]
    fn the_same_goal_in_the_same_program_replaces_the_older_recipe() {
        let dir = tempfile::tempdir().unwrap();
        let g = Guides::new(dir.path());
        g.save("save as pdf", "winword.exe", steps(), "1").unwrap();
        g.save("save as pdf", "winword.exe", steps()[..1].to_vec(), "2").unwrap();
        assert_eq!(g.list().len(), 1);
        assert_eq!(g.list()[0].steps.len(), 1);
    }

    #[test]
    fn empty_or_huge_recipes_and_path_like_ids_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let g = Guides::new(dir.path());
        assert!(g.save("x", "a.exe", vec![], "1").is_err());
        let many: Vec<RecipeStep> = (0..31).map(|_| steps()[0].clone()).collect();
        assert!(g.save("x", "a.exe", many, "1").is_err());
        for bad in ["", "..", "a/b", "a\\b", "x.json"] {
            assert!(g.delete(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_step_is_checked_against_the_live_window() {
        let s = steps();
        let live = Snapshot { elements: vec![el(1, "button", "Home", "HomeTab"), el(2, "button", "File", "FileTabButton"), el(3, "menuitem", "save as", "")], truncated: false };
        assert_eq!(s[0].find(&live).unwrap().id, 2, "by automation id");
        assert_eq!(s[1].find(&live).unwrap().id, 3, "by role and name, any case");
        let other = Snapshot { elements: vec![el(1, "button", "Home", "HomeTab")], truncated: false };
        assert!(s[0].find(&other).is_none() && s[1].find(&other).is_none(), "not in the window: the model takes over");
    }

    #[test]
    fn a_damaged_file_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let g = Guides::new(dir.path());
        std::fs::create_dir_all(dir.path().join("guides")).unwrap();
        std::fs::write(dir.path().join("guides").join("broken.json"), "{ not json").unwrap();
        g.save("x y", "a.exe", steps(), "1").unwrap();
        assert_eq!(g.list().len(), 1);
    }
}
