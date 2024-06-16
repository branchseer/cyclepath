mod parse_imports;
use std::{
    cell::RefCell,
    ffi::OsStr,
    io,
    path::{Component, Path},
    sync::Arc,
};

use bumpalo::Bump;
use hashbrown::{hash_map::DefaultHashBuilder, HashMap};
use oxc_allocator::Allocator;
use oxc_diagnostics::OxcDiagnostic;
use oxc_resolver::{FileSystem, ResolveOptions, ResolverGeneric};
use oxc_span::{SourceType, Span};
use smallvec::SmallVec;

use crate::collect_deps::DiscoverDependency;
use parse_imports::{parse_imports, Imports};
use thread_local::ThreadLocal;

#[derive(Debug)]
pub enum JsDiscoverDependencyError {
    FileReadError(io::Error),
    ParseOrResolveError {
        parse_errors: Vec<OxcDiagnostic>,
        resolve_errors: Vec<(oxc_resolver::ResolveError, Span)>,
        non_literal_imports: Vec<Span>,
    },
}

pub struct ResetOnDrop<'a>(&'a mut Allocator);
impl<'a> Drop for ResetOnDrop<'a> {
    fn drop(&mut self) {
        self.0.reset()
    }
}

pub struct JsDiscoverDependency<FS> {
    fs: FS,
    path_resolver: ResolverGeneric<FS>,
    allocator: ThreadLocal<RefCell<Allocator>>,
}
impl<FS: Clone + FileSystem> JsDiscoverDependency<FS> {
    pub fn new(fs: FS, resolve_options: ResolveOptions) -> Self {
        Self {
            fs: fs.clone(),
            path_resolver: ResolverGeneric::new_with_file_system(fs, resolve_options),
            allocator: ThreadLocal::new(),
        }
    }
}

impl<FS: FileSystem> DiscoverDependency for JsDiscoverDependency<FS> {
    type Edge = SmallVec<[Span; 1]>;

    type Error = JsDiscoverDependencyError;

    fn discover_dependencies(
        &self,
        file_path: &Path,
    ) -> (Vec<(Arc<Path>, Self::Edge)>, Option<Self::Error>) {
        let file_content = match self.fs.read_to_string(file_path) {
            Ok(ok) => ok,
            Err(err) => {
                return (vec![], Some(JsDiscoverDependencyError::FileReadError(err)));
            }
        };
        let allocator_ref_cell = self.allocator.get_or_default();
        let mut allocator_mut_ref: std::cell::RefMut<Allocator> = allocator_ref_cell.borrow_mut();
        let reset_on_drop = ResetOnDrop(&mut allocator_mut_ref);
        let allocator = &*reset_on_drop.0;

        let mut resolve_errors: Vec<(oxc_resolver::ResolveError, Span)> = vec![];
        let (
            Imports {
                specifiers,
                non_literal_imports,
            },
            parse_errors,
        ) = parse_imports(
            allocator,
            SourceType::from_path(file_path)
                .unwrap_or_else(|_| SourceType::default().with_jsx(true).with_module(true)),
            &file_content,
        );

        let mut spans_by_dep =
            HashMap::<Arc<Path>, SmallVec<[Span; 1]>, DefaultHashBuilder, &Bump>::with_capacity_in(
                specifiers.len(),
                allocator,
            );
        for (specifier, span) in specifiers {
            if !matches!(
                Path::new(specifier).components().next(),
                Some(Component::CurDir | Component::ParentDir)
            ) {
                // skip non-relative specifiers
                continue;
            }
            let resolution = match self
                .path_resolver
                .resolve(file_path.parent().unwrap_or(file_path), specifier)
            {
                Ok(ok) => ok,
                Err(err) => {
                    resolve_errors.push((err, span));
                    continue;
                }
            };
            let resolved_path = resolution.into_path_buf();
            if !matches!(
                resolved_path.extension().and_then(OsStr::to_str),
                Some("js" | "ts" | "jsx" | "tsx")
            ) {
                continue;
            }
            spans_by_dep
                .entry(resolved_path.into())
                .or_default()
                .push(span);
        }

        let error = if parse_errors.is_empty()
            && resolve_errors.is_empty()
            && non_literal_imports.is_empty()
        {
            None
        } else {
            Some(JsDiscoverDependencyError::ParseOrResolveError {
                parse_errors,
                resolve_errors,
                non_literal_imports,
            })
        };

        (spans_by_dep.into_iter().collect(), error)
    }
}
