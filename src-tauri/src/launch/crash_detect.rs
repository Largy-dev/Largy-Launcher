//! A small, deliberately dumb pattern table over the tail of a launch's
//! stdout/stderr — "does any buffered line contain a known crash
//! signature". Run once, at process exit (see [`super::orchestrator`]).

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CrashAnalysis {
    pub summary: String,
    pub suggestion: Option<String>,
    pub matched_pattern: &'static str,
    /// Path of the crash report Minecraft wrote, when it logged one.
    pub crash_report: Option<String>,
}

const MIXIN_SUMMARY: &str = "Un mod utilisant Mixin a échoué à s'appliquer — probablement une incompatibilité entre mods.";
const MIXIN_HINT: &str = "Vérifie que tous tes mods sont compatibles avec cette version de Minecraft et du mod loader.";

/// Ordered pattern table — first match wins, since generic signatures often
/// appear downstream of a more specific one.
const PATTERNS: &[(&str, &str, Option<&str>)] = &[
    (
        "Could not reserve enough space for",
        "Java n'a pas pu réserver la mémoire demandée.",
        Some("Baisse la RAM allouée à l'instance : ta machine n'a pas assez de mémoire libre."),
    ),
    (
        "Invalid maximum heap size",
        "La quantité de RAM configurée est invalide.",
        Some("Vérifie la RAM et les arguments JVM de l'instance."),
    ),
    (
        "Multiple garbage collectors selected",
        "Plusieurs ramasse-miettes (GC) sont activés en même temps dans les arguments JVM.",
        Some("N'en garde qu'un seul (G1GC ou ZGC) dans les paramètres Java de l'instance."),
    ),
    (
        "Unrecognized VM option",
        "Un argument JVM n'est pas reconnu par cette version de Java.",
        Some("Retire l'argument en cause dans les paramètres Java de l'instance."),
    ),
    (
        "java.lang.OutOfMemoryError",
        "Le jeu a manqué de mémoire.",
        Some("Augmente la RAM allouée dans les paramètres de l'instance."),
    ),
    (
        "UnsupportedClassVersionError",
        "Version de Java incompatible avec ce Minecraft ou un de ses mods.",
        Some("Laisse le launcher gérer Java automatiquement, ou choisis une version de Java plus récente."),
    ),
    (
        "Pixel format not accelerated",
        "Le pilote graphique ne prend pas en charge OpenGL.",
        Some("Mets à jour le pilote de ta carte graphique."),
    ),
    (
        "GLFW error 65542",
        "Le pilote graphique ne prend pas en charge OpenGL.",
        Some("Mets à jour le pilote de ta carte graphique."),
    ),
    (
        "java.lang.UnsatisfiedLinkError",
        "Une bibliothèque native n'a pas pu être chargée.",
        Some("Utilise « Réparer l'instance » pour retélécharger les fichiers du jeu."),
    ),
    ("Mixin apply failed", MIXIN_SUMMARY, Some(MIXIN_HINT)),
    ("MixinApplyError", MIXIN_SUMMARY, Some(MIXIN_HINT)),
    ("Missing or unsupported mandatory dependencies", "Des mods requis par d'autres mods sont manquants ou incompatibles.", None),
    ("requires any version of", "Des mods requis par d'autres mods sont manquants ou incompatibles.", None),
    (
        "Incompatible mods found!",
        "Des mods incompatibles entre eux sont installés.",
        Some("Le log ci-dessus indique quels mods retirer ou mettre à jour."),
    ),
    (
        "java.lang.NoClassDefFoundError",
        "Une classe est introuvable — un mod ou une de ses dépendances manque.",
        Some("Vérifie que les dépendances de tes mods sont installées."),
    ),
    ("A mod crashed on startup", "Un mod a provoqué un crash au démarrage du jeu.", None),
    (
        "Error occurred during initialization of VM",
        "Java a refusé de démarrer avant même le lancement du jeu.",
        Some("Vérifie les arguments JVM et la RAM de l'instance : la cause est indiquée juste en dessous dans la console."),
    ),
];

const CRASH_REPORT_MARKER: &str = "Crash report saved to:";

fn crash_report_path(log_lines: &[String]) -> Option<String> {
    log_lines.iter().rev().find_map(|line| {
        let (_, rest) = line.split_once(CRASH_REPORT_MARKER)?;
        let path = rest.trim().trim_start_matches("#@!@#").trim();
        (!path.is_empty()).then(|| path.to_string())
    })
}

/// `None` for a clean exit (code 0) or a launcher-initiated stop. A
/// non-zero, unrecognized exit still gets a generic explanation.
pub fn analyze(log_lines: &[String], exit_code: Option<i32>, killed: bool) -> Option<CrashAnalysis> {
    if killed || exit_code == Some(0) {
        return None;
    }
    let crash_report = crash_report_path(log_lines);
    for (needle, summary, suggestion) in PATTERNS {
        if log_lines.iter().any(|line| line.contains(needle)) {
            return Some(CrashAnalysis {
                summary: summary.to_string(),
                suggestion: suggestion.map(str::to_string),
                matched_pattern: needle,
                crash_report,
            });
        }
    }
    Some(CrashAnalysis {
        summary: match exit_code {
            Some(code) => format!("Le jeu s'est arrêté avec une erreur (code {code})."),
            None => "Le jeu s'est arrêté de manière inattendue.".to_string(),
        },
        suggestion: crash_report.is_some().then(|| "Consulte le crash report pour plus de détails.".to_string()),
        matched_pattern: "unknown-nonzero-exit",
        crash_report,
    })
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
        let analysis = analyze(&log, Some(1), false).unwrap();
        assert_eq!(analysis.matched_pattern, "java.lang.OutOfMemoryError");
        assert!(analysis.suggestion.is_some());
    }

    #[test]
    fn detects_mixin_failure() {
        let log = lines(&["[main/ERROR] Mixin apply failed examplemod.mixins.json:ExampleMixin -> net.minecraft.Foo"]);
        assert_eq!(analyze(&log, Some(1), false).unwrap().matched_pattern, "Mixin apply failed");
    }

    #[test]
    fn first_matching_pattern_wins() {
        let log = lines(&["java.lang.OutOfMemoryError", "Mixin apply failed"]);
        assert_eq!(analyze(&log, Some(1), false).unwrap().matched_pattern, "java.lang.OutOfMemoryError");
    }

    #[test]
    fn falls_back_to_generic_message_for_unrecognized_nonzero_exit() {
        let analysis = analyze(&lines(&["some totally normal line"]), Some(1), false).unwrap();
        assert_eq!(analysis.matched_pattern, "unknown-nonzero-exit");
        assert!(analysis.summary.contains('1'));
    }

    #[test]
    fn clean_exit_or_user_stop_is_never_a_crash() {
        let log = lines(&["java.lang.OutOfMemoryError mentioned in a harmless warning"]);
        assert_eq!(analyze(&log, Some(0), false), None);
        assert_eq!(analyze(&log, Some(1), true), None);
    }

    #[test]
    fn extracts_the_crash_report_path() {
        let log = lines(&[
            "#@!@# Game crashed! Crash report saved to: #@!@# C:\\games\\crash-reports\\crash-2026.txt",
        ]);
        let analysis = analyze(&log, Some(-1), false).unwrap();
        assert_eq!(analysis.crash_report.as_deref(), Some("C:\\games\\crash-reports\\crash-2026.txt"));
    }
}
