//! Language layouts and the entity indexes every resolution rule reads.
//!
//! The linker never guesses a language from a file name: each stable key written by the
//! tree-sitter engine carries the pack id, and the pack id selects the module layout (Python
//! modules are files, Go packages are directories) together with the own-instance keyword and the
//! test-file conventions. A pack id without a layout entry keeps resolving through the file layout
//! with the generic test convention, so adding a language to the engine does not silently drop its
//! references here.

use std::collections::HashMap;

use athanor_domain::{Entity, EntityKind};

pub(crate) const MODULE_SCHEME: &str = "module://";
pub(crate) const SYMBOL_SCHEME: &str = "symbol://";

/// Where the declarations of one language are reachable from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unit {
    /// A declaration is reachable from the file that holds it.
    File,
    /// A declaration is reachable from every file of its directory.
    Directory,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LanguageLayout {
    pub(crate) id: &'static str,
    pub(crate) unit: Unit,
    pub(crate) own_receiver: Option<&'static str>,
    pub(crate) module_extensions: &'static [&'static str],
    pub(crate) package_files: &'static [&'static str],
}

const LAYOUTS: &[LanguageLayout] = &[
    LanguageLayout {
        id: "python",
        unit: Unit::File,
        own_receiver: Some("self"),
        module_extensions: &["py", "pyi"],
        package_files: &["__init__.py", "__init__.pyi"],
    },
    LanguageLayout {
        id: "go",
        unit: Unit::Directory,
        own_receiver: None,
        module_extensions: &[],
        package_files: &[],
    },
];

const FALLBACK_LAYOUT: LanguageLayout = LanguageLayout {
    id: "",
    unit: Unit::File,
    own_receiver: None,
    module_extensions: &[],
    package_files: &[],
};

pub(crate) fn layout(language: &str) -> LanguageLayout {
    LAYOUTS
        .iter()
        .copied()
        .find(|layout| layout.id == language)
        .unwrap_or(FALLBACK_LAYOUT)
}

/// Splits `module://<language>:<path>`.
pub(crate) fn module_key(key: &str) -> Option<(&str, &str)> {
    key.strip_prefix(MODULE_SCHEME)?.split_once(':')
}

/// Splits `symbol://<language>:<path>#<qualified>`.
pub(crate) fn symbol_key(key: &str) -> Option<(&str, &str, &str)> {
    let rest = key.strip_prefix(SYMBOL_SCHEME)?;
    let (language, rest) = rest.split_once(':')?;
    let (path, qualified) = rest.split_once('#')?;
    Some((language, path, qualified))
}

pub(crate) fn directory_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(directory, _)| directory)
}

/// The resolution unit of one source path for a language.
pub(crate) fn unit_of(language: &str, path: &str) -> String {
    match layout(language).unit {
        Unit::File => path.to_string(),
        Unit::Directory => directory_of(path).to_string(),
    }
}

/// Reports whether a path is a test location for a language.
pub(crate) fn is_test_file(language: &str, path: &str) -> bool {
    let file_name = path.rsplit('/').next().unwrap_or(path);
    let convention = match language {
        "python" => {
            file_name.starts_with("test_")
                || file_name.ends_with("_test.py")
                || file_name.ends_with("_test.pyi")
        }
        "go" => file_name.ends_with("_test.go"),
        _ => false,
    };
    convention || has_test_directory(path)
}

fn has_test_directory(path: &str) -> bool {
    path.split('/')
        .any(|segment| matches!(segment, "tests" | "test" | "__tests__"))
}

fn is_declaration_kind(kind: &EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Function | EntityKind::Class | EntityKind::Symbol
    )
}

/// Declaration payload accessor shared by the cascade and the coverage report.
pub(crate) fn qualified_name(entity: &Entity) -> Option<&str> {
    entity
        .payload
        .get("qualified_name")
        .and_then(serde_json::Value::as_str)
}

pub(crate) fn is_exported(entity: &Entity) -> bool {
    entity
        .payload
        .get("exported")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

/// The modules and declarations of one snapshot, indexed for lookup.
pub(crate) struct Index<'a> {
    modules: Vec<&'a Entity>,
    declarations: Vec<&'a Entity>,
    module_by_path: HashMap<String, &'a Entity>,
    declarations_by_unit: HashMap<String, Vec<&'a Entity>>,
    declarations_by_name: HashMap<String, Vec<&'a Entity>>,
}

impl<'a> Index<'a> {
    pub(crate) fn build(entities: &'a [Entity]) -> Self {
        let mut modules = Vec::new();
        let mut declarations = Vec::new();
        let mut module_by_path = HashMap::new();
        let mut declarations_by_unit = HashMap::<String, Vec<&'a Entity>>::new();
        let mut declarations_by_name = HashMap::<String, Vec<&'a Entity>>::new();

        for entity in entities {
            if let Some((language, path)) = module_key(&entity.stable_key.0) {
                if entity.kind != EntityKind::Module {
                    continue;
                }
                modules.push(entity);
                module_by_path
                    .entry(format!("{language}:{path}"))
                    .or_insert(entity);
                continue;
            }
            let Some((language, path, _)) = symbol_key(&entity.stable_key.0) else {
                continue;
            };
            if !is_declaration_kind(&entity.kind) {
                continue;
            }
            declarations.push(entity);
            declarations_by_unit
                .entry(format!("{language}:{}", unit_of(language, path)))
                .or_default()
                .push(entity);
            declarations_by_name
                .entry(format!("{language}:{}", entity.name))
                .or_default()
                .push(entity);
        }

        modules.sort_by(|left, right| left.stable_key.0.cmp(&right.stable_key.0));
        declarations.sort_by(|left, right| left.stable_key.0.cmp(&right.stable_key.0));
        for candidates in declarations_by_unit.values_mut() {
            candidates.sort_by(|left, right| left.stable_key.0.cmp(&right.stable_key.0));
        }
        for candidates in declarations_by_name.values_mut() {
            candidates.sort_by(|left, right| left.stable_key.0.cmp(&right.stable_key.0));
        }

        Self {
            modules,
            declarations,
            module_by_path,
            declarations_by_unit,
            declarations_by_name,
        }
    }

    pub(crate) fn modules(&self) -> &[&'a Entity] {
        &self.modules
    }

    pub(crate) fn declarations(&self) -> &[&'a Entity] {
        &self.declarations
    }

    pub(crate) fn module(&self, language: &str, path: &str) -> Option<&'a Entity> {
        self.module_by_path
            .get(&format!("{language}:{path}"))
            .copied()
    }

    pub(crate) fn declarations_in(&self, language: &str, unit: &str) -> &[&'a Entity] {
        self.declarations_by_unit
            .get(&format!("{language}:{unit}"))
            .map_or(&[], Vec::as_slice)
    }

    pub(crate) fn declarations_named(&self, language: &str, name: &str) -> &[&'a Entity] {
        self.declarations_by_name
            .get(&format!("{language}:{name}"))
            .map_or(&[], Vec::as_slice)
    }
}
