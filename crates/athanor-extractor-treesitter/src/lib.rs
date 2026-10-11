//! Generic tree-sitter extraction behind the `Extractor` port.
//!
//! One adapter hosts every grammar the engine knows: a language is a [`LanguagePack`] of node
//! kinds, field names, and node-to-entity mappings, so adding a language is a pack module plus a
//! fixture rather than a new crate. The engine reads a file once, records the module entity, its
//! declarations, imports, exports, and statically named call sites, and discloses every bound that
//! truncated the result as a diagnostic.

mod engine;
mod packs;

pub use engine::{
    CallRule, DeclarationKindRule, DeclarationRule, ExportRule, ImportRule, LanguagePack,
    MAX_CALL_SITES_PER_DECLARATION, MAX_CALL_SITES_PER_FILE, MAX_DECLARATIONS_PER_FILE,
    MAX_EXPORTS_PER_FILE, MAX_IMPORTS_PER_FILE, MAX_PARSE_ERROR_DIAGNOSTICS, MAX_TRAVERSAL_DEPTH,
    MemberRule,
};
pub use packs::{go, language_packs, pack_for_path, python};

use async_trait::async_trait;
use athanor_core::{
    CoreResult, ExtractInput, ExtractOutput, Extractor, InvalidationPolicy, SourceFile,
};

/// Extracts from every hosted grammar, selected by file extension.
#[derive(Debug, Clone, Default)]
pub struct TreeSitterExtractor;

#[async_trait]
impl Extractor for TreeSitterExtractor {
    fn name(&self) -> &'static str {
        "treesitter"
    }

    fn invalidation_policy(&self) -> InvalidationPolicy {
        InvalidationPolicy::FILE_LOCAL
    }

    fn supports(&self, source: &SourceFile) -> bool {
        pack_for_path(&source.path).is_some()
    }

    async fn extract(&self, input: ExtractInput) -> CoreResult<ExtractOutput> {
        let Some(content) = input.source.content.as_deref() else {
            return Ok(ExtractOutput::default());
        };
        let Some(pack) = pack_for_path(&input.source.path) else {
            return Ok(ExtractOutput::default());
        };
        engine::extract_pack(pack, self.name(), &input, content)
    }
}

#[cfg(test)]
pub(crate) fn input(path: &str, content: &str, language_hint: &str) -> ExtractInput {
    ExtractInput {
        repo: athanor_domain::RepoId("repo_test".to_string()),
        snapshot: athanor_domain::SnapshotId("snap_test".to_string()),
        source: SourceFile {
            path: path.to_string(),
            language_hint: Some(language_hint.to_string()),
            content_hash: Some("hash".to_string()),
            content: Some(content.to_string()),
        },
    }
}
