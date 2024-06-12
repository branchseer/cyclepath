use oxc_allocator::Allocator;
use oxc_ast::{
    ast::{Argument, Expression, ImportOrExportKind},
    visit::{walk::walk_program, Visit},
};
use oxc_diagnostics::OxcDiagnostic;
use oxc_parser::Parser;
use oxc_span::GetSpan;
use oxc_span::{SourceType, Span};

pub fn get_deps<'a>(
    allocator: &'a Allocator,
    source: &'a str,
    on_dep: impl FnMut(&'a str),
    on_dynamic_import: impl FnMut(Span),
) -> Result<(), Vec<OxcDiagnostic>> {
    let parser = Parser::new(
        allocator,
        source,
        SourceType::default()
            .with_always_strict(true)
            .with_typescript(true)
            .with_jsx(true)
            .with_module(true),
    );
    let parse_return = parser.parse();
    if parse_return.panicked {
        return Err(parse_return.errors);
    }

    struct DepVisitor<OnDep, OnDynamicImport> {
        on_dep: OnDep,
        on_dynamic_import: OnDynamicImport,
    }
    impl<'a, OnDep: FnMut(&'a str), OnDynamicImport: FnMut(Span)> Visit<'a>
        for DepVisitor<OnDep, OnDynamicImport>
    {
        fn visit_import_declaration(&mut self, decl: &oxc_ast::ast::ImportDeclaration<'a>) {
            if decl.import_kind == ImportOrExportKind::Type {
                return;
            };
            (self.on_dep)(decl.source.value.as_str())
        }
        fn visit_ts_import_equals_declaration(
            &mut self,
            decl: &oxc_ast::ast::TSImportEqualsDeclaration<'a>,
        ) {
            if decl.import_kind == ImportOrExportKind::Type {
                return;
            };
            if let oxc_ast::ast::TSModuleReference::ExternalModuleReference(
                external_module_reference,
            ) = &decl.module_reference
            {
                (self.on_dep)(external_module_reference.expression.value.as_str())
            }
        }
        fn visit_import_expression(&mut self, expr: &oxc_ast::ast::ImportExpression<'a>) {
            if let Expression::StringLiteral(string_literal) = &expr.source {
                (self.on_dep)(string_literal.value.as_str())
            } else {
                (self.on_dynamic_import)(expr.source.span())
            }
        }
        fn visit_call_expression(&mut self, expr: &oxc_ast::ast::CallExpression<'a>) {
            if expr.arguments.len() != 1 {
                return;
            }
            let Expression::Identifier(callee_id) = &expr.callee else {
                return;
            };
            if callee_id.name != "require" {
                return;
            }
            let arg = &expr.arguments[0];
            if let Argument::StringLiteral(source) = arg {
                (self.on_dep)(source.value.as_str());
            } else {
                (self.on_dynamic_import)(arg.span());
            }
        }
    }
    walk_program(
        &mut DepVisitor {
            on_dep,
            on_dynamic_import,
        },
        &parse_return.program,
    );
    Ok(())
}

#[cfg(test)]
mod tests {

    use super::get_deps;
    use oxc_diagnostics::OxcDiagnostic;
    use oxc_span::Span;

    fn collect_deps(src: &str) -> Result<(Vec<String>, Vec<Span>), Vec<OxcDiagnostic>> {
        let mut deps: Vec<String> = vec![];
        let mut dynamic_import_spans: Vec<Span> = vec![];
        get_deps(
            &Default::default(),
            src,
            |dep| deps.push(dep.to_owned()),
            |span| dynamic_import_spans.push(span),
        )?;
        Ok((deps, dynamic_import_spans))
    }
    #[test]
    fn test_get_deps() {
        let src = "import 'foo';
import a from 'a';
import type b from 'b';
import c = require('c');
import type bar = require('bar');
const d = import('d');
const e = import('e' + d);
const f = require('f');
const g = require('g' + f);
";
        let (deps, dynamic_import_spans) = collect_deps(src).unwrap();
        assert_eq!(deps, vec!["foo", "a", "c", "d", "f"]);
        assert_eq!(
            dynamic_import_spans
                .into_iter()
                .map(|span| span.source_text(&src))
                .collect::<Vec<&str>>(),
            vec!["'e' + d", "'g' + f"]
        )
    }
}
