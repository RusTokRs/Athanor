//! The language packs hosted by the generic engine.

pub mod go;
pub mod python;

use crate::engine::LanguagePack;

/// Every hosted pack, in deterministic order.
static PACKS: &[&LanguagePack] = &[&python::PYTHON, &go::GO];

/// Returns every hosted language pack.
pub fn language_packs() -> &'static [&'static LanguagePack] {
    PACKS
}

/// Returns the pack that owns a file path.
pub fn pack_for_path(path: &str) -> Option<&'static LanguagePack> {
    PACKS.iter().copied().find(|pack| pack.supports(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_own_their_extensions_only() {
        assert_eq!(
            pack_for_path("api/client.py").map(|pack| pack.id),
            Some("python")
        );
        assert_eq!(
            pack_for_path("api/client.PY").map(|pack| pack.id),
            Some("python")
        );
        assert_eq!(
            pack_for_path("types/stubs.pyi").map(|pack| pack.id),
            Some("python")
        );
        assert_eq!(
            pack_for_path("cmd/server.go").map(|pack| pack.id),
            Some("go")
        );
        assert_eq!(pack_for_path("src/lib.rs"), None);
        assert_eq!(pack_for_path("Makefile"), None);
    }

    #[test]
    fn hosted_packs_keep_their_identity() {
        let ids = language_packs()
            .iter()
            .map(|pack| pack.id)
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["python", "go"]);
        for pack in language_packs() {
            assert!(pack.hint == pack.id, "hint must match the language id");
            assert!(!pack.extensions.is_empty());
            assert!(!pack.declarations.is_empty());
        }
    }
}
