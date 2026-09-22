//! A small, deliberately dumb pattern table over the tail of a launch's
//! stdout/stderr — not a state machine or multi-line correlation engine,
//! just "does any buffered line contain a known crash signature". Run once,
//! at process exit, against the buffered log (see [`super::orchestrator`]).

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CrashAnalysis {
    pub summary: String,
    pub suggestion: Option<String>,
    pub matched_pattern: &'static str,
}

/// Ordered pattern table — first match wins, since some signatures (e.g. a
/// generic mixin failure) are more common downstream of a more specific one
/// and would otherwise shadow it if checked in the wrong order.
const PATTERNS: &[(&str, &str, Option<&str>)] = &[
    (
        "java.lang.OutOfMemoryError",
        "Le jeu a manqué de mémoire.",
        Some("Augmente la RAM allouée dans les paramètres de l'instance."),
    ),
    (
        "UnsupportedClassVersionError",
        "Version de Java incompatible avec ce Minecraft.",
        Some("Vérifie le chemin Java personnalisé dans Paramètres, ou laisse le launcher gérer Java automatiquement."),
    ),
    (
        "Mixin apply failed",
        "Un mod utilisant Mixin a échoué à s'appliquer — probablement une incompatibilité entre mods.",
        Some("Vérifie que tous tes mods sont compatibles avec cette version de Minecraft et du mod loader."),
    ),
    (
        "MixinApplyError",
        "Un mod utilisant Mixin a échoué à s'appliquer — probablement une incompatibilité entre mods.",
        Some("Vérifie que tous tes mods sont compatibles avec cette version de Minecraft et du mod loader."),
    ),
    (
        "Missing or unsupported mandatory dependencies",
        "Des mods requis par d'autres mods sont manquants ou incompatibles.",
        None,
    ),
    ("A mod crashed on startup", "Un mod a provoqué un crash au démarrage du jeu.", None),
];

/// Scans `log_lines` (typically the last few hundred lines of a finished
/// launch) for a known crash signature. Returns `None` when nothing matched
/// and the exit was clean (`exit_code` is `Some(0)` or absent) — a non-zero,
/// unrecognized exit still gets a generic fallback so the user isn't left
/// with nothing.
pub fn analyze(log_lines: &[String], exit_code: Option<i32>) -> Option<CrashAnalysis> {
    for (needle, summary, suggestion) in PATTERNS {
        if log_lines.iter().any(|line| line.contains(needle)) {
            return Some(CrashAnalysis {
                summary: summary.to_string(),
                suggestion: suggestion.map(|s| s.to_string()),
                matched_pattern: needle,
            });
        }
    }

    match exit_code {
        Some(0) | None => None,
        Some(code) => Some(CrashAnalysis {
            summary: format!("Le jeu s'est arrêté avec une erreur (code {code})."),
            suggestion: None,
            matched_pattern: "unknown-nonzero-exit",
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn detects_out_of_memory() {
        let log = lines(&["Exception in thread \"main\" java.lang.OutOfMemoryError: Java heap space"]);
        let analysis = analyze(&log, Some(1)).unwrap();
        assert_eq!(analysis.matched_pattern, "java.lang.OutOfMemoryError");
        assert!(analysis.suggestion.is_some());
    }

    #[test]
    fn detects_mixin_failure() {
        let log = lines(&["[main/ERROR] Mixin apply failed examplemod.mixins.json:ExampleMixin -> net.minecraft.Foo"]);
        let analysis = analyze(&log, Some(1)).unwrap();
        assert_eq!(analysis.matched_pattern, "Mixin apply failed");
    }

    #[test]
    fn first_matching_pattern_wins() {
        let log = lines(&["java.lang.OutOfMemoryError", "Mixin apply failed"]);
        let analysis = analyze(&log, Some(1)).unwrap();
        assert_eq!(analysis.matched_pattern, "java.lang.OutOfMemoryError");
    }

    #[test]
    fn falls_back_to_generic_message_for_unrecognized_nonzero_exit() {
        let log = lines(&["some totally normal line"]);
        let analysis = analyze(&log, Some(1)).unwrap();
        assert_eq!(analysis.matched_pattern, "unknown-nonzero-exit");
        assert!(analysis.summary.contains('1'));
    }

    #[test]
    fn returns_none_for_a_clean_exit_with_no_known_signature() {
        let log = lines(&["Stopping server", "Saving worlds"]);
        assert_eq!(analyze(&log, Some(0)), None);
        assert_eq!(analyze(&log, None), None);
    }
}
