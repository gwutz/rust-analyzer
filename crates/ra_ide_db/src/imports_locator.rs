//! This module contains an import search funcionality that is provided to the ra_assists module.
//! Later, this should be moved away to a separate crate that is accessible from the ra_assists module.

use hir::{Crate, MacroDef, ModuleDef, Semantics};
use ra_prof::profile;
use ra_syntax::{ast, AstNode, SyntaxKind::NAME};

use crate::{
    defs::{classify_name, Definition},
    symbol_index::{self, FileSymbol, Query},
    RootDatabase,
};
use either::Either;
use rustc_hash::FxHashSet;

pub struct ImportsLocator<'a> {
    sema: Semantics<'a, RootDatabase>,
    krate: Crate,
}

impl<'a> ImportsLocator<'a> {
    pub fn new(db: &'a RootDatabase, krate: Crate) -> Self {
        Self { sema: Semantics::new(db), krate }
    }

    pub fn find_imports(&mut self, name_to_import: &str) -> Vec<Either<ModuleDef, MacroDef>> {
        let _p = profile("search_for_imports");
        let db = self.sema.db;

        // Query dependencies first.
        let mut candidates: FxHashSet<_> =
            self.krate.query_external_importables(db, name_to_import).collect();

        // Query the local crate using the symbol index.
        let local_results = {
            let mut query = Query::new(name_to_import.to_string());
            query.exact();
            query.limit(40);
            symbol_index::crate_symbols(db, self.krate.into(), query)
        };

        candidates.extend(
            local_results
                .into_iter()
                .filter_map(|import_candidate| self.get_name_definition(&import_candidate))
                .filter_map(|name_definition_to_import| match name_definition_to_import {
                    Definition::ModuleDef(module_def) => Some(Either::Left(module_def)),
                    Definition::Macro(macro_def) => Some(Either::Right(macro_def)),
                    _ => None,
                }),
        );

        candidates.into_iter().collect()
    }

    fn get_name_definition(&mut self, import_candidate: &FileSymbol) -> Option<Definition> {
        let _p = profile("get_name_definition");
        let file_id = import_candidate.file_id;

        let candidate_node = import_candidate.ptr.to_node(self.sema.parse(file_id).syntax());
        let candidate_name_node = if candidate_node.kind() != NAME {
            candidate_node.children().find(|it| it.kind() == NAME)?
        } else {
            candidate_node
        };
        let name = ast::Name::cast(candidate_name_node)?;
        classify_name(&self.sema, &name)?.into_definition()
    }
}
