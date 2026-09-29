use super::*;
use std::io::Write;

fn jar(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn read(bytes: Vec<u8>) -> LocalMeta {
    read_zip(&mut zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap(), true)
}

#[test]
fn fabric_mod_json_with_nested_jars() {
    let inner = jar(&[("fabric.mod.json", br#"{"id":"fabric-api-base","version":"1"}"#)]);
    let meta = read(jar(&[
        (
            "fabric.mod.json",
            br#"{"id":"sodium","name":"Sodium","version":"0.5.8","description":"Fast",
                "authors":["JellySquid", {"name":"IMS"}],"icon":{"16":"s.png","128":"assets/sodium/icon.png"},
                "provides":["embeddium-compat"],
                "depends":{"minecraft":"1.20.1","fabricloader":">=0.15","fabric-api-base":"*","indium":"*"}}"#,
        ),
        ("META-INF/jars/base.jar", &inner),
    ]));
    assert_eq!(meta.mod_id.as_deref(), Some("sodium"));
    assert_eq!(meta.name.as_deref(), Some("Sodium"));
    assert_eq!(meta.version.as_deref(), Some("0.5.8"));
    assert_eq!(meta.authors, vec!["JellySquid", "IMS"]);
    assert_eq!(meta.icon_entry.as_deref(), Some("assets/sodium/icon.png"));
    assert_eq!(meta.loaders, vec!["fabric"]);
    let deps = meta.deps_for(LoaderKind::Fabric).unwrap();
    assert_eq!(deps.provides, vec!["embeddium-compat", "fabric-api-base", "sodium"]);
    // Loader ids and ids the jar bundles itself are never "missing".
    assert_eq!(deps.depends, vec!["indium"]);
}

#[test]
fn quilt_mod_json() {
    let mut meta = LocalMeta::default();
    apply_quilt(
        &mut meta,
        r#"{"quilt_loader":{"id":"qsl","version":"7","metadata":{"name":"QSL","contributors":{"Ennui":"Owner"},
           "icon":"q.png"},"depends":["quilt_loader",{"id":"opt","optional":true},{"id":"needed"}]}}"#,
    );
    assert_eq!(meta.name.as_deref(), Some("QSL"));
    assert_eq!(meta.authors, vec!["Ennui"]);
    assert_eq!(meta.deps_for(LoaderKind::Quilt).unwrap().depends, vec!["quilt_loader", "needed"]);
}

#[test]
fn forge_mods_toml_resolves_the_jar_version_and_required_client_deps() {
    let toml = r#"
        modLoader="javafml"
        logoFile="logo.png"
        [[mods]]
        modId="create"
        version="${file.jarVersion}"
        displayName="Create"
        authors="simibubi, Zelo"
        description='''Building tools'''
        [[dependencies.create]]
        modId="forge"
        mandatory=true
        [[dependencies.create]]
        modId="flywheel"
        mandatory=true
        side="CLIENT"
        [[dependencies.create]]
        modId="jei"
        mandatory=false
        [[dependencies.create]]
        modId="serverthing"
        mandatory=true
        side="SERVER"
    "#;
    let meta = read(jar(&[
        ("META-INF/mods.toml", toml.as_bytes()),
        ("META-INF/MANIFEST.MF", b"Manifest-Version: 1.0\r\nImplementation-Version: 0.5.1f\r\n"),
    ]));
    assert_eq!(meta.version.as_deref(), Some("0.5.1f"));
    assert_eq!(meta.authors, vec!["simibubi", "Zelo"]);
    assert_eq!(meta.icon_entry.as_deref(), Some("logo.png"));
    assert_eq!(meta.loaders, vec!["forge"]);
    assert_eq!(meta.deps_for(LoaderKind::Forge).unwrap().depends, vec!["flywheel"]);
}

#[test]
fn neoforge_mods_toml_uses_dependency_types() {
    let mut meta = LocalMeta::default();
    apply_mods_toml(
        &mut meta,
        r#"[[mods]]
           modId="ae2"
           version="19.0"
           [[dependencies.ae2]]
           modId="guideme"
           type="required"
           [[dependencies.ae2]]
           modId="geckolib"
           [[dependencies.ae2]]
           modId="jei"
           type="optional"
           [[dependencies.ae2]]
           modId="embeddium"
           type="incompatible"
           versionRange="[0.0.1,)"
           [[dependencies.ae2]]
           modId="oldlib"
           type="incompatible"
           versionRange="(,1.6.2)""#,
        "neoforge",
        None,
    );
    assert_eq!(meta.version.as_deref(), Some("19.0"));
    let deps = meta.deps_for(LoaderKind::NeoForge).unwrap();
    assert_eq!(deps.depends, vec!["guideme", "geckolib"]);
    assert_eq!(
        deps.breaks,
        vec![
            BreakRule { id: "embeddium".into(), versions: None, maven: false },
            BreakRule { id: "oldlib".into(), versions: Some("(,1.6.2)".into()), maven: true },
        ]
    );
}

#[test]
fn multi_loader_jars_only_count_the_instance_loaders_dependencies() {
    let meta = read(jar(&[
        ("fabric.mod.json", br#"{"id":"collective","depends":{"fabric":"*"},"breaks":{"optifine":"*"}}"#),
        (
            "META-INF/neoforge.mods.toml",
            br#"[[mods]]
                modId="collective"
                [[dependencies.collective]]
                modId="neoforge"
                type="required""#,
        ),
    ]));
    assert_eq!(meta.loaders, vec!["fabric", "neoforge"]);
    let neo = meta.deps_for(LoaderKind::NeoForge).unwrap();
    assert!(neo.depends.is_empty() && neo.breaks.is_empty());
    assert_eq!(meta.deps_for(LoaderKind::Quilt).unwrap().depends, vec!["fabric"]);
    assert_eq!(meta.deps_for(LoaderKind::Fabric).unwrap().breaks, vec![BreakRule::any("optifine".into())]);
    assert!(meta.deps_for(LoaderKind::Forge).is_none());
    assert!(meta.deps_for(LoaderKind::Vanilla).is_none());
}

#[test]
fn fabric_break_predicates_are_kept() {
    let mut meta = LocalMeta::default();
    apply_fabric(&mut meta, r#"{"id":"a","breaks":{"b":"<0.5","c":["1.0","2.0"],"d":"*"}}"#);
    let breaks = &meta.deps_for(LoaderKind::Fabric).unwrap().breaks;
    assert_eq!(breaks[0].versions.as_deref(), Some("<0.5"));
    assert_eq!(breaks[1].versions.as_deref(), Some("1.0 || 2.0"));
    assert_eq!(breaks[2], BreakRule::any("d".into()));
}

#[test]
fn a_bare_library_jar_provides_the_mods_it_bundles() {
    let inner = jar(&[("META-INF/neoforge.mods.toml", b"[[mods]]\nmodId=\"kotlinforforge\"\n")]);
    let meta = read(jar(&[
        ("META-INF/MANIFEST.MF", b"FMLModType: LIBRARY\r\n"),
        ("META-INF/jarjar/kffmod.jar", &inner),
    ]));
    assert_eq!(meta.deps_for(LoaderKind::NeoForge).unwrap().provides, vec!["kotlinforforge"]);
}

#[test]
fn legacy_mcmod_info() {
    let meta = read(jar(&[(
        "mcmod.info",
        br#"[{"modid":"jei","name":"Just Enough Items","version":"4.16","authorList":["mezz"]}]"#,
    )]));
    assert_eq!(meta.name.as_deref(), Some("Just Enough Items"));
    assert_eq!(meta.authors, vec!["mezz"]);
    assert_eq!(meta.loaders, vec!["forge"]);
}

#[test]
fn resource_pack_description_is_flattened_and_unformatted() {
    let meta = read(jar(&[
        ("pack.mcmeta", r#"{"pack":{"pack_format":15,"description":[{"text":"§6Faithful "},"32x"]}}"#.as_bytes()),
        ("pack.png", b"png"),
    ]));
    assert_eq!(meta.description.as_deref(), Some("Faithful 32x"));
    assert_eq!(meta.icon_entry.as_deref(), Some("pack.png"));
    assert!(meta.loaders.is_empty());
}

#[test]
fn broken_files_give_empty_metadata() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("x.jar"), b"not a zip").unwrap();
    assert_eq!(read_archive(&dir.path().join("x.jar")), LocalMeta::default());
    assert_eq!(read(jar(&[("fabric.mod.json", b"{oops")])).loaders, Vec::<String>::new());
}
