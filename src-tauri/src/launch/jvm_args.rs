//! Last line of defence before `java` starts: user JVM arguments (global
//! defaults + the instance's own) that would stop the JVM from booting are
//! fixed or dropped here, and every change is explained in the game console.
//! A JVM that refuses its arguments exits before Minecraft writes any log,
//! so without this the player only sees "crash, no log".

/// What was changed and why — shown in the console before the game's output.
pub type Notes = Vec<String>;

fn is_gc_selector(arg: &str) -> bool {
    arg.strip_prefix("-XX:+Use").is_some_and(|rest| rest.ends_with("GC"))
}

/// `-Xmx4G` / `-Xms512m` / `-Xmx1073741824` → bytes.
fn heap_size(value: &str) -> Option<u64> {
    let (digits, unit) = match value.char_indices().last()? {
        (i, c) if c.is_ascii_alphabetic() => (&value[..i], c.to_ascii_lowercase()),
        _ => (value, 'b'),
    };
    let n: u64 = digits.parse().ok()?;
    let shift = match unit {
        'b' => 0,
        'k' => 10,
        'm' => 20,
        'g' => 30,
        't' => 40,
        _ => return None,
    };
    n.checked_mul(1 << shift)
}

/// Options that take their value as the next token (`--add-opens x=y`).
fn takes_separate_value(arg: &str) -> bool {
    arg.starts_with("--") && !arg.contains('=')
}

/// `extra` are the user's arguments, appended after the launcher's own
/// `-Xms{min_mb}M -Xmx{max_mb}M` (so a user `-Xmx` overrides the slider).
pub fn sanitize(extra: Vec<String>, java_major: u32, min_mb: u32, max_mb: u32) -> (Vec<String>, Notes) {
    let mut notes = Notes::new();

    // A bare word would be taken as the main class ("Could not find or load
    // main class"): usually a stray `java`, or a path split on its spaces.
    let mut args = Vec::with_capacity(extra.len());
    let mut value_expected = false;
    for arg in extra {
        if value_expected || arg.starts_with('-') {
            value_expected = !value_expected && takes_separate_value(&arg);
            args.push(arg);
        } else {
            notes.push(format!(
                "« {arg} » ignoré : ce n'est pas un argument JVM (ils commencent tous par « - »). \
                 Un chemin contenant des espaces doit être évité."
            ));
        }
    }

    // ZGC exists from Java 15 and its generational mode from Java 21; an
    // older JVM rejects the flag outright. From 24 generational is the only
    // mode and the flag just prints a deprecation warning.
    let too_old = |arg: &str| match arg {
        "-XX:+UseZGC" => java_major < 15,
        "-XX:+ZGenerational" | "-XX:-ZGenerational" => java_major < 21,
        _ => false,
    };
    if args.iter().any(|a| too_old(a)) {
        notes.push(format!(
            "ZGC ignoré : il n'est pas pris en charge par Java {java_major}, utilisé par cette version de Minecraft."
        ));
        args.retain(|a| !too_old(a));
    }
    if java_major >= 24 {
        args.retain(|a| a != "-XX:+ZGenerational");
    }

    // Two collectors → "Multiple garbage collectors selected". The last one
    // wins: the instance's args come after the global defaults.
    if let Some(keep) = args.iter().rev().find(|a| is_gc_selector(a)).cloned() {
        let mut dropped: Vec<&String> = args.iter().filter(|a| is_gc_selector(a) && **a != keep).collect();
        dropped.sort();
        dropped.dedup();
        if !dropped.is_empty() {
            notes.push(format!(
                "Plusieurs ramasse-miettes (GC) étaient activés : seul {keep} est gardé, {} ignoré. \
                 Java refuse de démarrer avec plus d'un GC.",
                dropped.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
            ));
            args.retain(|a| !is_gc_selector(a) || *a == keep);
        }
    }

    // A minimum heap above the maximum → "Initial heap size set to a larger
    // value than the maximum heap size". The maximum is what the player
    // cares about, so the minimum gives way.
    let last_size =
        |args: &[String], prefix: &str| args.iter().rev().find_map(|a| a.strip_prefix(prefix).and_then(heap_size));
    let xms = last_size(&args, "-Xms").unwrap_or(u64::from(min_mb) << 20);
    let xmx = last_size(&args, "-Xmx").unwrap_or(u64::from(max_mb) << 20);
    if xms > xmx {
        args.retain(|a| !a.starts_with("-Xms"));
        args.push(format!("-Xms{}M", xmx >> 20));
        notes.push(format!(
            "La RAM minimale ({} Mo) dépassait la RAM maximale ({} Mo) : le minimum est ramené au maximum.",
            xms >> 20,
            xmx >> 20
        ));
    }

    (args, notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn clean_args_pass_through_untouched() {
        let args = strings(&["-XX:+UseG1GC", "-Dfoo=bar", "--add-opens", "java.base/java.lang=ALL-UNNAMED"]);
        let (out, notes) = sanitize(args.clone(), 21, 1024, 4096);
        assert_eq!(out, args);
        assert!(notes.is_empty());
    }

    #[test]
    fn keeps_only_the_last_garbage_collector() {
        let args = strings(&[
            "-XX:+UseG1GC",
            "-XX:MaxGCPauseMillis=200",
            "-XX:+UseZGC",
            "-XX:+ZGenerational",
            "-XX:+UseGCOverheadLimit",
        ]);
        let (out, notes) = sanitize(args, 21, 1024, 4096);
        assert_eq!(
            out,
            strings(&["-XX:MaxGCPauseMillis=200", "-XX:+UseZGC", "-XX:+ZGenerational", "-XX:+UseGCOverheadLimit"])
        );
        assert_eq!(notes.len(), 1);
    }

    #[test]
    fn repeated_same_gc_is_not_a_conflict() {
        let (out, notes) = sanitize(strings(&["-XX:+UseG1GC", "-XX:+UseG1GC"]), 21, 1024, 4096);
        assert_eq!(out.len(), 2);
        assert!(notes.is_empty());
    }

    #[test]
    fn drops_zgc_on_a_java_too_old_for_it() {
        let (out, notes) = sanitize(strings(&["-XX:+UseZGC", "-XX:+ZGenerational", "-Dfoo=bar"]), 17, 1024, 4096);
        assert_eq!(out, strings(&["-XX:+UseZGC", "-Dfoo=bar"]));
        assert_eq!(notes.len(), 1);
        let (out, _) = sanitize(strings(&["-XX:+UseZGC"]), 8, 1024, 4096);
        assert!(out.is_empty());
    }

    #[test]
    fn silently_drops_the_obsolete_generational_flag_on_java_24_plus() {
        let (out, notes) = sanitize(strings(&["-XX:+UseZGC", "-XX:+ZGenerational"]), 25, 1024, 4096);
        assert_eq!(out, strings(&["-XX:+UseZGC"]));
        assert!(notes.is_empty());
    }

    #[test]
    fn drops_bare_words_but_keeps_separate_option_values() {
        let args = strings(&["java", "-Dfoo=bar", "--add-exports", "a/b=ALL-UNNAMED", "Files\\x"]);
        let (out, notes) = sanitize(args, 21, 1024, 4096);
        assert_eq!(out, strings(&["-Dfoo=bar", "--add-exports", "a/b=ALL-UNNAMED"]));
        assert_eq!(notes.len(), 2);
    }

    #[test]
    fn caps_the_minimum_heap_at_the_maximum() {
        let (out, notes) = sanitize(strings(&["-Xmx2G"]), 21, 4096, 8192);
        assert_eq!(out, strings(&["-Xmx2G", "-Xms2048M"]));
        assert_eq!(notes.len(), 1);
        let (out, notes) = sanitize(strings(&["-Xms1g", "-Xmx4096m"]), 21, 1024, 8192);
        assert_eq!(out, strings(&["-Xms1g", "-Xmx4096m"]));
        assert!(notes.is_empty());
    }

    #[test]
    fn parses_heap_sizes() {
        assert_eq!(heap_size("4G"), Some(4 << 30));
        assert_eq!(heap_size("512m"), Some(512 << 20));
        assert_eq!(heap_size("1024"), Some(1024));
        assert_eq!(heap_size("lots"), None);
    }
}
