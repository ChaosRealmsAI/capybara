fn load_theme_css(root: &Path, project_slug: &str, theme_id: &str) -> Result<String, ProjectError> {
    let theme_dir = root.join(project_slug).join("themes").join(theme_id);
    let mut css = String::new();
    for file in ["tokens.css", "theme.css", "components.css"] {
        let path = theme_dir.join(file);
        if path.exists() {
            let raw = fs::read_to_string(&path).map_err(|err| {
                ProjectError::StorageFailed(format!(
                    "theme CSS read failed: {}: {err}",
                    path.display()
                ))
            })?;
            css.push_str(&raw);
            css.push('\n');
        }
    }
    Ok(css)
}

fn load_component_js(
    root: &Path,
    project_slug: &str,
    component_id: &str,
) -> Result<String, ProjectError> {
    validate_component_id(component_id)?;
    let package_path = component_package_path(root, project_slug, component_id);
    if package_path.join("component.json").is_file() {
        return capy_components::load_component_package(&package_path)
            .map(|package| package.runtime)
            .map_err(|err| ProjectError::ValidationFailed(err.to_string()));
    }
    let path = component_source_path(root, project_slug, component_id);
    fs::read_to_string(&path).map_err(|err| {
        ProjectError::StorageFailed(format!(
            "component source read failed: {}: {err}",
            path.display()
        ))
    })
}

fn validate_component_id(component_id: &str) -> Result<(), ProjectError> {
    capy_components::validate_component_id(component_id)
        .map_err(|err| ProjectError::ValidationFailed(err.to_string()))
}

fn component_package_path(root: &Path, project_slug: &str, component_id: &str) -> PathBuf {
    root.join(project_slug).join("components").join(component_id)
}

fn component_source_path(root: &Path, project_slug: &str, component_id: &str) -> PathBuf {
    root.join(project_slug)
        .join("components")
        .join(format!("{component_id}.js"))
}

fn inspect_component_source(
    root: &Path,
    project_slug: &str,
    component_id: &str,
    errors: &mut Vec<String>,
) -> ComponentValidationComponent {
    let package_path = component_package_path(root, project_slug, component_id);
    if package_path.join("component.json").is_file() {
        match capy_components::load_component_package(&package_path) {
            Ok(package) => {
                let exports = component_exports_from(package.runtime.as_str());
                return ComponentValidationComponent {
                    id: component_id.to_string(),
                    path: package.manifest_path.display().to_string(),
                    exists: true,
                    bytes: package.runtime.len(),
                    exports,
                    params: Vec::new(),
                    used_by: Vec::new(),
                };
            }
            Err(err) => {
                errors.push(err.to_string());
                return ComponentValidationComponent {
                    id: component_id.to_string(),
                    path: package_path.display().to_string(),
                    exists: false,
                    bytes: 0,
                    exports: ComponentExports::default(),
                    params: Vec::new(),
                    used_by: Vec::new(),
                };
            }
        }
    }
    let path = component_source_path(root, project_slug, component_id);
    let display_path = path.display().to_string();
    let Ok(source) = fs::read_to_string(&path) else {
        errors.push(format!(
            "component source missing: {project_slug}/components/{component_id}.js"
        ));
        return ComponentValidationComponent {
            id: component_id.to_string(),
            path: display_path,
            exists: false,
            bytes: 0,
            exports: ComponentExports::default(),
            params: Vec::new(),
            used_by: Vec::new(),
        };
    };
    let exports = inspect_component_exports(&source);
    ComponentValidationComponent {
        id: component_id.to_string(),
        path: display_path,
        exists: true,
        bytes: source.len(),
        exports,
        params: Vec::new(),
        used_by: Vec::new(),
    }
}

fn component_exports_from(source: &str) -> ComponentExports {
    let exports = capy_components::inspect_component_exports(source);
    ComponentExports {
        mount: exports.mount,
        update: exports.update,
        destroy: exports.destroy,
        imports: exports.imports,
        dynamic_imports: exports.dynamic_imports,
    }
}

fn inspect_component_exports(source: &str) -> ComponentExports {
    component_exports_from(source)
}

fn list_project_components(root: &Path, project_slug: &str) -> Result<Vec<String>, ProjectError> {
    let dir = root.join(project_slug).join("components");
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut components = std::collections::BTreeSet::new();
    for entry in fs::read_dir(&dir)
        .map_err(|err| ProjectError::StorageFailed(format!("components read failed: {err}")))?
    {
        let entry = entry.map_err(|err| {
            ProjectError::StorageFailed(format!("components entry read failed: {err}"))
        })?;
        let path = entry.path();
        if path.is_dir() && path.join("component.json").is_file() {
            if let Some(id) = path.file_name().and_then(|value| value.to_str()) {
                if validate_component_id(id).is_ok() {
                    components.insert(id.to_string());
                }
            }
        } else if path.extension().and_then(|value| value.to_str()) == Some("js") {
            if let Some(id) = path.file_stem().and_then(|value| value.to_str()) {
                if validate_component_id(id).is_ok() {
                    components.insert(id.to_string());
                }
            }
        }
    }
    Ok(components.into_iter().collect())
}

fn copy_number_param(
    source: &serde_json::Map<String, Value>,
    target: &mut serde_json::Map<String, Value>,
    key: &str,
) {
    if let Some(value) = source.get(key).and_then(Value::as_f64) {
        target.insert(key.to_string(), json!(value));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Episode, JsonStorage, Project, Registry, RegistryProject, Storage,
        compile_composition_source, validate_composition_components,
    };

    #[test]
    fn registry_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let storage = test_storage("registry")?;
        let registry = Registry {
            projects: vec![RegistryProject {
                slug: "next-frame".to_string(),
                name: "Timeline".to_string(),
                created: "2026-04-21T00:00:00Z".to_string(),
                last_modified: "2026-04-21T00:00:00Z".to_string(),
            }],
        };

        storage.save_registry(&registry)?;
        let loaded = storage.load_registry()?;

        assert_eq!(loaded, registry);
        cleanup(storage.root())?;
        Ok(())
    }

    #[test]
    fn project_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let storage = test_storage("project")?;
        let project = Project {
            slug: "demo-video".to_string(),
            name: "Demo Video".to_string(),
            description: Some("Demo".to_string()),
            tags: Some(vec!["demo".to_string()]),
            created: "2026-04-21T00:00:00Z".to_string(),
            modified: "2026-04-21T00:00:00Z".to_string(),
        };
        let episode = Episode {
            slug: "ep-01".to_string(),
            name: "Episode 01".to_string(),
            duration: 60.0,
            anchors: Default::default(),
            clips: Vec::new(),
            log: Vec::new(),
        };

        storage.save_project(&project)?;
        storage.save_episode(&project.slug, &episode)?;

        assert_eq!(storage.load_project(&project.slug)?, project);
        assert_eq!(storage.load_episode(&project.slug, &episode.slug)?, episode);
        cleanup(storage.root())?;
        Ok(())
    }

    #[test]
    fn compiles_scene_clips_to_source_json() -> Result<(), Box<dyn std::error::Error>> {
        let episode = Episode {
            slug: "ep-01".to_string(),
            name: "Episode 01".to_string(),
            duration: 5.0,
            anchors: Default::default(),
            clips: vec![
                serde_json::json!({
                    "slug": "intro",
                    "label": "Hello Capybara Timeline",
                    "track": "scene",
                    "start": "0",
                    "end": "5",
                    "position": {"x": 42.0, "y": 58.0}
                }),
                serde_json::json!({
                    "slug": "caption",
                    "label": "Text overlay",
                    "track": "text",
                    "start": "0",
                    "end": "5",
                    "position": {"x": 50.0, "y": 82.0}
                }),
            ],
            log: Vec::new(),
        };

        let compiled = super::compile_episode_source("demo", &episode)?;

        assert_eq!(compiled.source["duration"], 5000);
        assert_eq!(
            compiled.source["tracks"][0]["clips"][0]["params"]["title"],
            "Hello Capybara Timeline"
        );
        assert_eq!(
            compiled.source["tracks"][0]["clips"][0]["params"]["title_x"],
            42.0
        );
        assert_eq!(
            compiled.source["tracks"][0]["clips"][0]["params"]["title_y"],
            58.0
        );
        assert_eq!(compiled.source["tracks"][1]["kind"], "text");
        assert_eq!(
            compiled.source["tracks"][1]["clips"][0]["params"]["text"],
            "Text overlay"
        );
        Ok(())
    }

    #[test]
    fn compiles_v2_composition_components() -> Result<(), Box<dyn std::error::Error>> {
        let storage = test_storage("composition")?;
        let project_dir = storage.root().join("demo");
        std::fs::create_dir_all(project_dir.join("components"))?;
        std::fs::create_dir_all(project_dir.join("themes").join("launch.dark"))?;
        std::fs::write(
            project_dir.join("components").join("html.hero-title.js"),
            "export function mount() {}\nexport function update() {}\n",
        )?;
        std::fs::write(
            project_dir
                .join("themes")
                .join("launch.dark")
                .join("tokens.css"),
            ":root { --accent: #62f5d2; }\n",
        )?;
        let composition = serde_json::json!({
            "id": "launch-open",
            "name": "Launch Open",
            "duration": "4s",
            "theme": "launch.dark",
            "anchors": { "in": "0s", "out": "4s" },
            "tracks": [{
                "id": "hero",
                "kind": "component",
                "component": "html.hero-title",
                "time": { "start": "in", "end": "out" },
                "params": { "title": "Hello" }
            }, {
                "id": "voice",
                "kind": "audio",
                "time": { "start": "in", "end": "out" },
                "src": "audio/demo.mp3",
                "volume": 0.8
            }]
        });

        let compiled = compile_composition_source(&storage, "demo", &composition)?;

        assert_eq!(compiled.source["duration"], 4000);
        assert_eq!(compiled.source["tracks"][0]["kind"], "component");
        assert_eq!(compiled.source["tracks"][1]["kind"], "audio");
        assert!(
            compiled.source["tracks"][1]["clips"][0]["params"]["src"]
                .as_str()
                .unwrap_or_default()
                .starts_with("file://")
        );
        assert!(
            compiled.source["tracks"][1]["clips"][0]["params"]["src"]
                .as_str()
                .unwrap_or_default()
                .ends_with("/demo/audio/demo.mp3")
        );
        assert_eq!(
            compiled.source["tracks"][0]["clips"][0]["params"]["component"],
            "html.hero-title"
        );
        assert!(
            compiled.source["components"]["html.hero-title"]
                .as_str()
                .unwrap_or_default()
                .contains("update")
        );
        assert!(
            compiled.source["theme"]["css"]
                .as_str()
                .unwrap_or_default()
                .contains("--accent")
        );
        cleanup(storage.root())?;
        Ok(())
    }

    #[test]
    fn validates_component_registry_contract() -> Result<(), Box<dyn std::error::Error>> {
        let storage = test_storage("component-contract")?;
        let project_dir = storage.root().join("demo");
        std::fs::create_dir_all(project_dir.join("components"))?;
        std::fs::write(
            project_dir.join("components").join("html.hero-title.js"),
            "export function mount() {}\nexport function update() {}\n",
        )?;
        let composition = serde_json::json!({
            "id": "launch-open",
            "name": "Launch Open",
            "duration": "4s",
            "tracks": [{
                "id": "hero",
                "kind": "component",
                "component": "html.hero-title",
                "time": { "start": "0s", "end": "4s" },
                "style": { "x": 50 },
                "params": { "title": "Hello" }
            }]
        });

        let report = validate_composition_components(&storage, "demo", &composition)?;

        assert!(report.ok, "{:?}", report.errors);
        assert_eq!(report.available_components, vec!["html.hero-title"]);
        assert_eq!(report.components[0].id, "html.hero-title");
        assert_eq!(report.components[0].params, vec!["title", "x"]);
        assert!(report.components[0].exports.mount);
        assert!(report.components[0].exports.update);
        cleanup(storage.root())?;
        Ok(())
    }

    #[test]
    fn validates_component_package_contract() -> Result<(), Box<dyn std::error::Error>> {
        let storage = test_storage("component-package-contract")?;
        let component_dir = storage
            .root()
            .join("demo")
            .join("components")
            .join("html.hero-title");
        std::fs::create_dir_all(&component_dir)?;
        std::fs::write(
            component_dir.join("component.json"),
            r#"{
              "schema": "capy.component.v1",
              "id": "html.hero-title",
              "version": "0.1.0",
              "surfaces": ["video", "poster", "web"],
              "entrypoints": { "runtime": "runtime.js" },
              "trusted": true
            }"#,
        )?;
        std::fs::write(
            component_dir.join("runtime.js"),
            "export function mount() {}\nexport function update() {}\n",
        )?;
        let composition = serde_json::json!({
            "id": "launch-open",
            "name": "Launch Open",
            "duration": "4s",
            "tracks": [{
                "id": "hero",
                "kind": "component",
                "component": "html.hero-title",
                "time": { "start": "0s", "end": "4s" },
                "params": { "title": "Hello" }
            }]
        });

        let report = validate_composition_components(&storage, "demo", &composition)?;
        let compiled = compile_composition_source(&storage, "demo", &composition)?;

        assert!(report.ok, "{:?}", report.errors);
        assert_eq!(report.components[0].path, component_dir.join("component.json").display().to_string());
        assert!(
            compiled.source["components"]["html.hero-title"]
                .as_str()
                .unwrap_or_default()
                .contains("export function update")
        );
        cleanup(storage.root())?;
        Ok(())
    }

    #[test]
    fn rejects_component_without_update_export() -> Result<(), Box<dyn std::error::Error>> {
        let storage = test_storage("component-contract-fail")?;
        let project_dir = storage.root().join("demo");
        std::fs::create_dir_all(project_dir.join("components"))?;
        std::fs::write(
            project_dir.join("components").join("html.hero-title.js"),
            "export function mount() {}\n",
        )?;
        let composition = serde_json::json!({
            "id": "launch-open",
            "name": "Launch Open",
            "duration": "4s",
            "tracks": [{
                "id": "hero",
                "kind": "component",
                "component": "html.hero-title",
                "time": { "start": "0s", "end": "4s" }
            }]
        });

        let report = validate_composition_components(&storage, "demo", &composition)?;

        assert!(!report.ok);
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.contains("missing export function update"))
        );
        cleanup(storage.root())?;
        Ok(())
    }

    fn test_storage(label: &str) -> Result<JsonStorage, Box<dyn std::error::Error>> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("timeline-{label}-{}-{nanos}", std::process::id()));
        Ok(JsonStorage::new(path))
    }

    fn cleanup(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        if path.exists() {
            std::fs::remove_dir_all(path)?;
        }
        Ok(())
    }
}
